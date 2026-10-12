use std::fs;
use std::path::PathBuf;
#[cfg(not(windows))]
use std::process::Command;

#[cfg(not(windows))]
use crate::config::set_mode;
use crate::{Error, Result};

/// Windows starts BlakSync at login from the per-user Run key. Unlike a
/// Startup-folder `.cmd`, that launches `blaksync.exe` directly, so no
/// `cmd.exe` window is left behind and the tray closes its own console.
#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "BlakSync";

#[cfg(not(windows))]
pub fn status() -> Result<bool> {
    Ok(unit_path()?.is_file())
}

#[cfg(windows)]
pub fn status() -> Result<bool> {
    Ok(crate::win::hkcu_string_get(RUN_KEY, RUN_VALUE)?.is_some()
        || legacy_startup_cmd()?.is_file())
}

#[cfg(not(windows))]
pub fn enable() -> Result<PathBuf> {
    let path = unit_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_mode(parent, 0o700)?;
    }
    fs::write(&path, unit_contents()?)?;
    set_mode(&path, 0o600)?;
    activate(&path)?;
    Ok(path)
}

#[cfg(windows)]
pub fn enable() -> Result<PathBuf> {
    crate::win::hkcu_string_set(RUN_KEY, RUN_VALUE, &unit_contents()?)?;
    remove_legacy_startup_cmd()?;
    Ok(PathBuf::from(format!(
        r"HKEY_CURRENT_USER\{RUN_KEY}\{RUN_VALUE}"
    )))
}

#[cfg(not(windows))]
pub fn disable() -> Result<bool> {
    let path = unit_path()?;
    deactivate(&path)?;
    if path.is_file() {
        fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(windows)]
pub fn disable() -> Result<bool> {
    let removed = crate::win::hkcu_value_delete(RUN_KEY, RUN_VALUE)?;
    Ok(remove_legacy_startup_cmd()? || removed)
}

/// Earlier builds wrote `BlakSync.cmd` to the Startup folder.
#[cfg(windows)]
fn legacy_startup_cmd() -> Result<PathBuf> {
    let startup = dirs::data_dir()
        .ok_or_else(|| Error::Config("Could not resolve the Windows startup folder.".into()))?
        .join("Microsoft/Windows/Start Menu/Programs/Startup");
    Ok(startup.join("BlakSync.cmd"))
}

#[cfg(windows)]
fn remove_legacy_startup_cmd() -> Result<bool> {
    let path = legacy_startup_cmd()?;
    if path.is_file() {
        fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(not(windows))]
pub fn unit_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home =
            dirs::home_dir().ok_or_else(|| Error::Config("Could not resolve home.".into()))?;
        Ok(home.join("Library/LaunchAgents/au.com.yumait.blaksync.plist"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let config = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not resolve the user config directory.".into()))?;
        Ok(config.join("systemd/user/blaksync.service"))
    }
}

fn unit_contents() -> Result<String> {
    let exe = current_exe()?;
    #[cfg(target_os = "macos")]
    {
        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>au.com.yumait.blaksync</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>tray</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#,
            exe.display()
        ))
    }
    #[cfg(windows)]
    {
        Ok(format!("\"{}\" tray", exe.display()))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Ok(format!(
            "[Unit]\nDescription=BlakSync tray and Syncthing wrapper\nDocumentation=https://github.com/yumaitau/BlakSync\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart={} tray\nRestart=on-failure\nRestartSec=5s\n\n[Install]\nWantedBy=default.target\n",
            exe.display()
        ))
    }
}

#[cfg(not(windows))]
fn activate(path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("launchctl")
            .args(["unload", &path.display().to_string()])
            .status();
        let status = Command::new("launchctl")
            .args(["load", &path.display().to_string()])
            .status();
        let _ = status;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let _ = Command::new("systemctl")
            .args(["--user", "enable", "--now", "blaksync.service"])
            .status();
        Ok(())
    }
}

#[cfg(not(windows))]
fn deactivate(path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("launchctl")
            .args(["unload", &path.display().to_string()])
            .status();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "blaksync.service"])
            .status();
        Ok(())
    }
}

fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_contents_do_not_embed_an_api_key() {
        let contents = unit_contents().unwrap();
        assert!(contents.contains("tray"));
        assert!(!contents.contains("API_KEY"));
        assert!(!contents.contains("apikey"));
    }
}
