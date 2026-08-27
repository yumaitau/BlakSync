use serde::Deserialize;

use crate::version::blaksync_version;
use crate::{Error, Result};

const DEFAULT_RELEASES_URL: &str = "https://api.github.com/repos/yumaitau/BlakSync/releases/latest";

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub enabled: bool,
    pub current: String,
    pub latest: Option<String>,
    pub newer: bool,
    pub release_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub fn default_update_url() -> String {
    std::env::var("BLAKSYNC_UPDATE_URL").unwrap_or_else(|_| DEFAULT_RELEASES_URL.into())
}

pub fn assert_update_host(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| Error::Config("Version check URL is not valid.".into()))?;
    let host = parsed.host_str().unwrap_or_default();
    let allowed_loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let allowed_github =
        host == "api.github.com" && parsed.path().starts_with("/repos/yumaitau/BlakSync/");
    if !allowed_loopback && !allowed_github {
        return Err(Error::Config(
            "Version check may only use yumaitau/BlakSync releases or a local test fixture.".into(),
        ));
    }
    Ok(())
}

pub async fn check_for_update(enabled: bool) -> Result<UpdateInfo> {
    let current = blaksync_version().to_string();
    if !enabled {
        return Ok(UpdateInfo {
            enabled: false,
            current,
            latest: None,
            newer: false,
            release_url: None,
        });
    }
    let url = default_update_url();
    assert_update_host(&url)?;
    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "BlakSync")
        .header("Accept", "application/json")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Config(format!(
            "Version check failed ({})",
            response.status()
        )));
    }
    let release: GithubRelease = response.json().await?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    Ok(UpdateInfo {
        enabled: true,
        current: current.clone(),
        newer: is_newer(&latest, &current),
        latest: Some(latest),
        release_url: Some(release.html_url),
    })
}

fn is_newer(latest: &str, current: &str) -> bool {
    parse_parts(latest) > parse_parts(current)
}

fn parse_parts(value: &str) -> [u64; 3] {
    let mut parts = [0; 3];
    for (index, piece) in value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|piece| !piece.is_empty())
        .take(3)
        .enumerate()
    {
        parts[index] = piece.parse().unwrap_or(0);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_allows_yuma_or_loopback_hosts() {
        assert!(assert_update_host(DEFAULT_RELEASES_URL).is_ok());
        assert!(assert_update_host("http://127.0.0.1:9/latest").is_ok());
        assert!(assert_update_host("https://example.invalid/v1").is_err());
        assert!(
            assert_update_host(
                "https://api.github.com/repos/someone-else/BlakSync/releases/latest"
            )
            .is_err()
        );
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
    }
}
