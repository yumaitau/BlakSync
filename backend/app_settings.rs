use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::ConfigStore;
use crate::{Error, Result};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryPreset {
    Lan,
    #[default]
    Global,
    Tailscale,
}

impl DiscoveryPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lan => "lan",
            Self::Global => "global",
            Self::Tailscale => "tailscale",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lan => "LAN only",
            Self::Global => "Defaults (LAN plus global discovery)",
            Self::Tailscale => "Tailscale only",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "lan" | "lan-only" | "lan_only" => Ok(Self::Lan),
            "global" | "defaults" | "default" => Ok(Self::Global),
            "tailscale" | "tailscale-only" | "tailscale_only" => Ok(Self::Tailscale),
            _ => Err(Error::Config(
                "Discovery preset must be lan, global, or tailscale.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub discovery_preset: DiscoveryPreset,
    #[serde(default)]
    pub tailscale_listen: String,
    #[serde(default)]
    pub version_check: bool,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub start_at_login: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            discovery_preset: DiscoveryPreset::Global,
            tailscale_listen: String::new(),
            version_check: false,
            tls: false,
            start_at_login: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppSettingsStore {
    store: ConfigStore,
}

impl AppSettingsStore {
    pub fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: ConfigStore::new(config_dir)?,
        })
    }

    pub fn load(&self) -> Result<AppSettings> {
        self.store.read_json(SETTINGS_FILE, AppSettings::default())
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        self.store
            .with_lock(|| self.store.write_json(SETTINGS_FILE, settings))
    }

    pub fn update(&self, mutate: impl FnOnce(&mut AppSettings)) -> Result<AppSettings> {
        self.store.with_lock(|| {
            let mut settings = self
                .store
                .read_json(SETTINGS_FILE, AppSettings::default())?;
            mutate(&mut settings);
            self.store.write_json(SETTINGS_FILE, &settings)?;
            Ok(settings)
        })
    }
}

/// Tailscale-only must listen on a concrete address, never a public wildcard.
pub fn validate_tailscale_listen(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Error::Config(
            "Tailscale-only needs a listen address such as tcp://100.64.0.1:22000.".into(),
        ));
    }
    let lowered = value.to_ascii_lowercase();
    let forbidden = [
        "0.0.0.0", "[::]", "tcp://:", "quic://:", "dynamic", "default",
    ];
    if forbidden.iter().any(|item| lowered.contains(item))
        || lowered == "tcp://22000"
        || lowered.starts_with("tcp://:")
        || lowered.starts_with("quic://:")
    {
        return Err(Error::Config(
            "Tailscale-only refuses a public wildcard listen address. Set the Tailscale IP.".into(),
        ));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_validates_tailscale_listen() {
        let temporary = tempfile::tempdir().unwrap();
        let store = AppSettingsStore::new(temporary.path()).unwrap();
        store
            .update(|settings| {
                settings.discovery_preset = DiscoveryPreset::Lan;
                settings.version_check = false;
            })
            .unwrap();
        assert_eq!(store.load().unwrap().discovery_preset, DiscoveryPreset::Lan);
        assert!(validate_tailscale_listen("tcp://0.0.0.0:22000").is_err());
        assert!(validate_tailscale_listen("tcp://100.64.1.8:22000").is_ok());
    }
}
