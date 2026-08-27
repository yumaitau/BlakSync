use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

const LOCK_FILE: &str = ".lock";

#[derive(Clone, Debug)]
pub struct ConfigStore {
    root: PathBuf,
}

impl ConfigStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if root.as_os_str().is_empty() {
            return Err(Error::Config("A config directory is required.".into()));
        }
        Ok(Self {
            root: absolute(root)?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        set_mode(&self.root, 0o700)?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path(name).is_file()
    }

    pub fn with_lock<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        self.ensure_dir()?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(self.path(LOCK_FILE))?;
        lock.lock_exclusive()?;
        let result = operation();
        FileExt::unlock(&lock)?;
        result
    }

    pub fn read_json<T: DeserializeOwned>(&self, name: &str, default: T) -> Result<T> {
        let path = self.path(name);
        if !path.is_file() {
            return Ok(default);
        }
        Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
    }

    pub fn read_optional_json<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        let path = self.path(name);
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_reader(BufReader::new(File::open(
            path,
        )?))?))
    }

    pub fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        self.ensure_dir()?;
        let path = self.path(name);
        let temporary = self.path(&format!("{name}.tmp"));
        let mut file = secure_file(&temporary)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn append_json_line<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        self.ensure_dir()?;
        let path = self.path(name);
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn read_json_lines<T: DeserializeOwned>(&self, name: &str) -> Result<Vec<T>> {
        let path = self.path(name);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let mut values = Vec::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            if !line.trim().is_empty() {
                values.push(serde_json::from_str(&line)?);
            }
        }
        Ok(values)
    }
}

pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

pub fn extract_apikey(xml: &str) -> Result<String> {
    let start = xml
        .find("<apikey>")
        .ok_or_else(|| Error::Config("config.xml does not contain a GUI API key yet.".into()))?;
    let rest = &xml[start + "<apikey>".len()..];
    let end = rest
        .find("</apikey>")
        .ok_or_else(|| Error::Config("config.xml has a broken <apikey> tag.".into()))?;
    let key = rest[..end].trim();
    if key.is_empty() {
        return Err(Error::Config("config.xml has an empty <apikey>.".into()));
    }
    Ok(key.to_string())
}

pub fn read_syncthing_api_key(home: &Path) -> Result<String> {
    let path = home.join("config.xml");
    let text = fs::read_to_string(&path).map_err(|_| {
        Error::Config(
            "Syncthing has not written config.xml yet. Run `blaksync start` first.".into(),
        )
    })?;
    extract_apikey(&text)
}

/// Environment override for tests; otherwise the key in `config.xml`.
pub fn resolve_api_key(home: &Path) -> Result<String> {
    if let Ok(key) = env::var("BLAKSYNC_API_KEY") {
        if !key.trim().is_empty() {
            return Ok(key);
        }
    }
    read_syncthing_api_key(home)
}

pub fn default_config_dir() -> PathBuf {
    if let Some(value) = env::var_os("BLAKSYNC_CONFIG_DIR") {
        return PathBuf::from(value);
    }
    let base = dirs::config_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
    });
    #[cfg(target_os = "macos")]
    let name = "BlakSync";
    #[cfg(not(target_os = "macos"))]
    let name = "blaksync";
    base.join(name)
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if let Ok(relative) = path.strip_prefix("~") {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Config("Could not resolve the home directory.".into()))?;
        return Ok(home.join(relative));
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(env::current_dir()?.join(path))
}

fn secure_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_apikey_without_echoing_it_in_errors() {
        let key = extract_apikey("<gui><apikey>local-test-key-12345678901</apikey></gui>").unwrap();
        assert_eq!(key, "local-test-key-12345678901");
        let error = extract_apikey("<gui></gui>").unwrap_err().to_string();
        assert!(error.contains("API key"));
        assert!(!error.contains("local-test-key"));
    }
}
