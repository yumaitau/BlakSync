use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::{default_config_dir, set_mode};
use crate::{Error, Result};

const BACKUP_NAMES: &[&str] = &[
    "cert.pem",
    "key.pem",
    "https-cert.pem",
    "https-key.pem",
    "config.xml",
    "org.json",
    "roles.json",
    "folders.json",
    "pending-devices.json",
    "devices.json",
    "audit.jsonl",
    "access-notes.json",
    "settings.json",
];

pub fn backup(config_dir: Option<&Path>, out: &Path) -> Result<PathBuf> {
    let source = absolute(config_dir.unwrap_or(&default_config_dir()))?;
    let dest = absolute(out)?;
    refuse_repo_path(&dest)?;
    fs::create_dir_all(&dest)?;
    set_mode(&dest, 0o700)?;
    for name in BACKUP_NAMES {
        let from = source.join(name);
        if from.is_file() {
            copy_private(&from, &dest.join(name))?;
        }
    }
    copy_ignore_files(&source, &dest)?;
    Ok(dest)
}

pub fn restore(config_dir: Option<&Path>, from: &Path) -> Result<PathBuf> {
    let dest = absolute(config_dir.unwrap_or(&default_config_dir()))?;
    let source = absolute(from)?;
    if !source.is_dir() {
        return Err(Error::Config(
            "Restore needs a backup directory created by `blaksync backup`.".into(),
        ));
    }
    refuse_repo_path(&dest)?;
    fs::create_dir_all(&dest)?;
    set_mode(&dest, 0o700)?;
    for name in BACKUP_NAMES {
        let from = source.join(name);
        if from.is_file() {
            copy_private(&from, &dest.join(name))?;
        }
    }
    let ignores = source.join("ignores");
    if ignores.is_dir() {
        let dest_ignores = dest.join("ignores");
        fs::create_dir_all(&dest_ignores)?;
        set_mode(&dest_ignores, 0o700)?;
        for entry in fs::read_dir(&ignores)? {
            let entry = entry?;
            if entry.path().is_file() {
                copy_private(&entry.path(), &dest_ignores.join(entry.file_name()))?;
            }
        }
    }
    Ok(dest)
}

pub fn is_inside_blaksync_repo(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor.join(".git").exists()
            && ancestor.join("PRODUCT.md").is_file()
            && ancestor.join("backend/lib.rs").is_file()
    })
}

fn refuse_repo_path(path: &Path) -> Result<()> {
    let probe = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    if is_inside_blaksync_repo(&probe) || is_inside_blaksync_repo(path) {
        return Err(Error::Config(
            "Refusing to write a config backup inside the BlakSync git repository. Use org-owned disk or a USB drive."
                .into(),
        ));
    }
    Ok(())
}

fn copy_ignore_files(source: &Path, dest: &Path) -> Result<()> {
    let dest_ignores = dest.join("ignores");
    let xml = source.join("config.xml");
    if xml.is_file() {
        let text = fs::read_to_string(xml)?;
        for folder_path in folder_paths_from_config(&text) {
            let ignore = folder_path.join(".stignore");
            if ignore.is_file() {
                fs::create_dir_all(&dest_ignores)?;
                set_mode(&dest_ignores, 0o700)?;
                let name = folder_path
                    .file_name()
                    .map(|name| format!("{}.stignore", name.to_string_lossy()))
                    .unwrap_or_else(|| "folder.stignore".into());
                copy_private(&ignore, &dest_ignores.join(name))?;
            }
        }
    }
    if source.join("ignores").is_dir() {
        fs::create_dir_all(&dest_ignores)?;
        for entry in WalkDir::new(source.join("ignores")).max_depth(2) {
            let entry = entry.map_err(io::Error::other)?;
            if entry.file_type().is_file() {
                copy_private(entry.path(), &dest_ignores.join(entry.file_name()))?;
            }
        }
    }
    Ok(())
}

fn folder_paths_from_config(xml: &str) -> Vec<PathBuf> {
    xml.split("path=\"")
        .skip(1)
        .filter_map(|chunk| chunk.split('"').next())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn copy_private(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
        set_mode(parent, 0o700)?;
    }
    fs::copy(from, to)?;
    set_mode(to, 0o600)?;
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_round_trip_and_repo_refusal() {
        let temporary = tempfile::tempdir().unwrap();
        let config = temporary.path().join("config");
        let out = temporary.path().join("usb");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("cert.pem"), "CERT").unwrap();
        fs::write(config.join("key.pem"), "KEY").unwrap();
        fs::write(config.join("config.xml"), "<configuration/>").unwrap();
        backup(Some(&config), &out).unwrap();
        assert!(out.join("cert.pem").is_file());

        let restored = temporary.path().join("restored");
        restore(Some(&restored), &out).unwrap();
        assert_eq!(
            fs::read_to_string(restored.join("cert.pem")).unwrap(),
            "CERT"
        );

        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/blaksync-backup-should-fail");
        let error = backup(Some(&config), &repo).unwrap_err().to_string();
        assert!(error.contains("git repository"));
        assert!(!error.contains("KEY"));
    }
}
