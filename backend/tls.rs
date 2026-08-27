use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use rcgen::generate_simple_self_signed;

use crate::config::set_mode;
use crate::{Error, Result};

pub const CERT_NAME: &str = "https-cert.pem";
pub const KEY_NAME: &str = "https-key.pem";

pub fn cert_paths(config_dir: &Path) -> (PathBuf, PathBuf) {
    (config_dir.join(CERT_NAME), config_dir.join(KEY_NAME))
}

pub fn ensure_local_certificate(config_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(config_dir)?;
    set_mode(config_dir, 0o700)?;
    let (cert_path, key_path) = cert_paths(config_dir);
    if cert_path.is_file() && key_path.is_file() {
        return Ok((cert_path, key_path));
    }
    let certified = generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
        .map_err(|error| {
            Error::Config(format!(
                "Could not create a local HTTPS certificate: {error}"
            ))
        })?;
    std::fs::write(&cert_path, certified.cert.pem())?;
    std::fs::write(&key_path, certified.key_pair.serialize_pem())?;
    set_mode(&cert_path, 0o600)?;
    set_mode(&key_path, 0o600)?;
    Ok((cert_path, key_path))
}

pub async fn serve_tls(
    listener: std::net::TcpListener,
    app: Router,
    config_dir: &Path,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let (cert_path, key_path) = ensure_local_certificate(config_dir)?;
    let rustls = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .map_err(|error| Error::Config(format!("Could not load HTTPS certificate: {error}")))?;
    let handle = axum_server::Handle::new();
    let stop = handle.clone();
    tokio::spawn(async move {
        shutdown.await;
        stop.graceful_shutdown(Some(Duration::from_secs(2)));
    });
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    axum_server::from_tcp_rustls(listener.into_std()?, rustls)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .map_err(|error| Error::Config(format!("HTTPS listener failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_gitignored_local_cert_names() {
        let temporary = tempfile::tempdir().unwrap();
        let (cert, key) = ensure_local_certificate(temporary.path()).unwrap();
        assert_eq!(cert.file_name().unwrap(), "https-cert.pem");
        assert_eq!(key.file_name().unwrap(), "https-key.pem");
        assert!(
            std::fs::read_to_string(cert)
                .unwrap()
                .contains("BEGIN CERTIFICATE")
        );
        assert!(
            std::fs::read_to_string(key)
                .unwrap()
                .contains("PRIVATE KEY")
        );
    }
}
