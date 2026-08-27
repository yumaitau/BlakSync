use crate::{Error, Result};

/// Supported Syncthing release. Bump this together with `scripts/install-syncthing.sh`.
pub const PINNED_SYNCTHING: &str = "2.1.3";

/// CLI / `--version` footer. Keep in lockstep with `PINNED_SYNCTHING`.
pub const LONG_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\nSyncthing pin: 2.1.3");

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SyncthingVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SyncthingVersion {
    pub fn parse(text: &str) -> Result<Self> {
        let re = regex::Regex::new(r"v?(\d+)\.(\d+)\.(\d+)").expect("version regex");
        let caps = re.captures(text).ok_or_else(|| {
            Error::Process(format!(
                "Could not read a Syncthing version from: {}",
                text.lines().next().unwrap_or(text).trim()
            ))
        })?;
        Ok(Self {
            major: caps[1].parse().unwrap_or(0),
            minor: caps[2].parse().unwrap_or(0),
            patch: caps[3].parse().unwrap_or(0),
        })
    }

    pub fn pinned() -> Result<Self> {
        Self::parse(PINNED_SYNCTHING)
    }

    pub fn display(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Refuse Syncthing binaries that are not the pinned major series.
pub fn ensure_supported(version: SyncthingVersion) -> Result<()> {
    let pin = SyncthingVersion::pinned()?;
    if version.major != pin.major {
        return Err(Error::Process(format!(
            "Unsupported Syncthing {}. BlakSync is pinned to Syncthing {} (major {}). Install the bundled binary or see docs/syncthing-pin.md.",
            version.display(),
            PINNED_SYNCTHING,
            pin.major
        )));
    }
    Ok(())
}

pub fn blaksync_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_syncthing_version_lines() {
        let version = SyncthingVersion::parse("syncthing v2.1.3 \"Gold Grasshopper\"").unwrap();
        assert_eq!(version.display(), "2.1.3");
        ensure_supported(version).unwrap();
    }

    #[test]
    fn refuses_wrong_major() {
        let old = SyncthingVersion::parse("v1.29.7").unwrap();
        let error = ensure_supported(old).unwrap_err().to_string();
        assert!(error.contains("Unsupported Syncthing 1.29.7"));
        assert!(error.contains(PINNED_SYNCTHING));
    }
}
