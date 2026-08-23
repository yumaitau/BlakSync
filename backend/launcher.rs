use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::config::default_config_dir;
use crate::org::require_device_id;
use crate::{Error, Result};

const GUI_ADDRESS: &str = "127.0.0.1:8384";

#[derive(Clone, Debug)]
pub struct LaunchResult {
    pub device_id: String,
    pub gui_url: String,
    pub home: PathBuf,
    pub exit_code: i32,
}

pub async fn launch_syncthing(binary: Option<&str>, home: Option<&Path>) -> Result<LaunchResult> {
    let binary = binary
        .filter(|value| !value.is_empty())
        .unwrap_or("syncthing");
    let default_home = default_config_dir();
    let home = absolute(home.unwrap_or(&default_home))?;
    tokio::fs::create_dir_all(&home).await?;
    set_private_directory(&home)?;

    if !home.join("config.xml").is_file() {
        run_output(binary, &generate_args(&home)).await?;
    }
    let device_id = run_output(binary, &device_id_args(&home)).await?;
    let device_id = require_device_id(device_id.trim())?;

    println!("Syncthing GUI: http://{GUI_ADDRESS}");
    println!("Device ID: {device_id}");
    println!("BlakSync config: {}", home.display());

    let status = Command::new(binary)
        .args(serve_args(&home))
        .env("STVERSIONEXTRA", "BlakSync")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|error| executable_error(binary, error))?;

    Ok(LaunchResult {
        device_id,
        gui_url: format!("http://{GUI_ADDRESS}"),
        home,
        exit_code: status.code().unwrap_or(1),
    })
}

fn generate_args(home: &Path) -> Vec<String> {
    vec![
        "generate".into(),
        format!("--home={}", home.display()),
        "--no-port-probing".into(),
    ]
}

fn device_id_args(home: &Path) -> Vec<String> {
    vec!["device-id".into(), format!("--home={}", home.display())]
}

fn serve_args(home: &Path) -> Vec<String> {
    vec![
        "serve".into(),
        format!("--home={}", home.display()),
        format!("--gui-address={GUI_ADDRESS}"),
        "--no-browser".into(),
        "--no-port-probing".into(),
    ]
}

async fn run_output(binary: &str, args: &[String]) -> Result<String> {
    let output = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| executable_error(binary, error))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Process(format!(
            "Syncthing {} failed ({}){}",
            args.first().map(String::as_str).unwrap_or("command"),
            output
                .status
                .code()
                .map_or_else(|| "unknown".into(), |code| code.to_string()),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn executable_error(binary: &str, error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        return Error::Process(format!(
            "Syncthing executable not found: {binary}. Install Syncthing or pass --syncthing PATH."
        ));
    }
    Error::Io(error)
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn set_private_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_current_syncthing_cli_contract() {
        let home = Path::new("/tmp/blaksync");
        assert_eq!(generate_args(home)[0], "generate");
        assert!(generate_args(home).contains(&"--no-port-probing".into()));
        assert_eq!(device_id_args(home)[0], "device-id");
        assert_eq!(
            serve_args(home),
            vec![
                "serve",
                "--home=/tmp/blaksync",
                "--gui-address=127.0.0.1:8384",
                "--no-browser",
                "--no-port-probing",
            ]
        );
    }
}
