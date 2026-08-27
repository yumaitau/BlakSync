use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use regex::Regex;
use tempfile::TempDir;
use walkdir::{DirEntry, WalkDir};

use crate::{Error, Result};

const BEGIN: &str = "-----BEGIN";
const END: &str = "-----END";
const PRIVATE: &str = "PRIVATE KEY";
const CERTIFICATE: &str = "CERTIFICATE";

static PEM_PRIVATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)-----BEGIN[ A-Z0-9_-]{0,100}PRIVATE KEY(?: BLOCK)?-----[\s\S-]{64,}?KEY(?: BLOCK)?-----",
    )
    .expect("valid private-key regex")
});
static PEM_CERT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)-----BEGIN[ A-Z0-9_-]{0,40}CERTIFICATE-----").expect("valid certificate regex")
});
static API_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<apikey>\s*([A-Za-z0-9_-]{22,64})\s*</apikey>").expect("valid API-key regex")
});
static API_KEY_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(REDACTED|YOUR[_-]?GUI[_-]?API[_-]?KEY|changeme|placeholder)$")
        .expect("valid placeholder regex")
});

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Finding {
    pub rule: String,
    pub snippet: String,
}

pub fn findings_in(path: &str, content: &str) -> Vec<Finding> {
    if skipped(path) {
        return Vec::new();
    }

    let mut findings = Vec::new();
    let normalised = path.replace('\\', "/");
    let file_name = normalised.rsplit('/').next().unwrap_or(&normalised);
    let is_config_xml = file_name.eq_ignore_ascii_case("config.xml");

    if let Some(found) = PEM_PRIVATE.find(content) {
        findings.push(Finding {
            rule: "private-key".into(),
            snippet: redacted(found.as_str(), 24),
        });
    }
    if matches!(
        file_name.to_ascii_lowercase().as_str(),
        "key.pem" | "https-key.pem"
    ) && content.contains(BEGIN)
    {
        findings.push(Finding {
            rule: "syncthing-device-key-file".into(),
            snippet: normalised.clone(),
        });
    }
    if matches!(
        file_name.to_ascii_lowercase().as_str(),
        "cert.pem" | "https-cert.pem"
    ) && PEM_CERT.is_match(content)
    {
        findings.push(Finding {
            rule: "syncthing-device-cert-file".into(),
            snippet: normalised,
        });
    }
    if is_config_xml {
        for captures in API_KEY.captures_iter(content) {
            let value = captures.get(1).map_or("", |item| item.as_str());
            if !API_KEY_PLACEHOLDER.is_match(value) {
                findings.push(Finding {
                    rule: "syncthing-gui-apikey".into(),
                    snippet: "<apikey>…[redacted]…</apikey>".into(),
                });
            }
        }
    }
    findings
}

pub fn scan_repo(root: &Path) -> Result<Vec<String>> {
    let revisions = revisions(root);
    if revisions.is_empty() {
        return scan_working_tree(root);
    }

    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    for revision in revisions {
        let short = revision.chars().take(12).collect::<String>();
        for path in tracked_paths(root, &revision)? {
            let Some(content) = blob_at(root, &revision, &path)? else {
                continue;
            };
            for finding in findings_in(&path, &content) {
                if seen.insert((path.clone(), finding.clone())) {
                    lines.push(format!(
                        "{short}:{path}: {}: {}",
                        finding.rule, finding.snippet
                    ));
                }
            }
        }
    }
    Ok(lines)
}

pub fn dummy_private_key() -> String {
    let body = concat!(
        "MHcCAQEEIBlakSyncCiDummyKeyNotSecretAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
        "oAoGCCqGSM49AwEHoUQDQgAEDummyKeyForCiDetectionOnlyNotASecretXX\n",
        "BlakSyncIssue6DummyKeyPemMustFailTheScanXXXXXXXXXXXXXXXXXXXX==\n"
    );
    format!("{BEGIN} EC {PRIVATE}-----\n{body}{END} EC {PRIVATE}-----\n")
}

pub fn self_test() -> Result<()> {
    assert_detects("dummy key.pem", "key.pem", &dummy_private_key())?;
    assert_detects("dummy https-key.pem", "https-key.pem", &dummy_private_key())?;
    assert_detects(
        "dummy cert.pem",
        "cert.pem",
        &format!("{BEGIN} {CERTIFICATE}-----\nMIIBDummyCertNotADeviceIdentityXXXXXXXX\n"),
    )?;
    assert_detects(
        "live config.xml API key",
        "config.xml",
        &dummy_api_key_config(),
    )?;
    assert_clean(
        "docs mentioning key.pem",
        "README.md",
        "Never commit cert.pem, key.pem, or config.xml with API keys.\n",
    )?;
    assert_clean(
        "placeholder API key",
        "config.example.xml",
        "<apikey>REDACTED</apikey>\n",
    )?;
    println!("self-test passed: dummy keys fail; docs do not");
    Ok(())
}

pub fn write_dummy_key(path: PathBuf) -> Result<()> {
    fs::write(path, dummy_private_key())?;
    Ok(())
}

fn scan_working_tree(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(walk_entry)
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        let content = fs::read(path)?;
        let content = String::from_utf8_lossy(&content);
        for finding in findings_in(&relative, &content) {
            lines.push(format!("{relative}: {}: {}", finding.rule, finding.snippet));
        }
    }
    Ok(lines)
}

fn walk_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    !matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | "node_modules" | "target"
    )
}

fn revisions(root: &Path) -> Vec<String> {
    git_output(root, &["rev-list", "HEAD"])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn tracked_paths(root: &Path, revision: &str) -> Result<Vec<String>> {
    let output = checked_git(root, &["ls-tree", "-r", "--name-only", "-z", revision])?;
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}

fn blob_at(root: &Path, revision: &str, path: &str) -> Result<Option<String>> {
    let object = format!("{revision}:{path}");
    let output = git_output(root, &["show", &object])?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

fn skipped(path: &str) -> bool {
    let normalised = path.replace('\\', "/").to_ascii_lowercase();
    normalised == ".gitleaks.toml"
        || normalised.ends_with("/.gitleaks.toml")
        || normalised == "backend/security_scan.rs"
        || normalised.ends_with("/backend/security_scan.rs")
}

fn redacted(value: &str, limit: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= limit {
        let kept = compact.chars().take((limit / 2).max(4)).collect::<String>();
        return format!("{kept}…");
    }
    let start = compact.chars().take(12).collect::<String>();
    let end = compact
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{start}…[redacted]…{end}")
}

fn dummy_api_key_config() -> String {
    let tag = "apikey";
    let value = ["k1dnz1Dd0rzTBjjFFh7", "CXPnrF12C49B1"].concat();
    format!(
        "<configuration version=\"37\">\n  <gui>\n    <address>127.0.0.1:8384</address>\n    <{tag}>{value}</{tag}>\n  </gui>\n</configuration>\n"
    )
}

fn assert_detects(label: &str, relative: &str, content: &str) -> Result<()> {
    let fixture = git_fixture(relative, content)?;
    let hits = scan_repo(fixture.path())?;
    if hits.is_empty() {
        return Err(Error::Process(format!(
            "self-test failed: {label} was not detected"
        )));
    }
    println!("ok: {label} failed the scan ({} finding(s))", hits.len());
    Ok(())
}

fn assert_clean(label: &str, relative: &str, content: &str) -> Result<()> {
    let fixture = git_fixture(relative, content)?;
    let hits = scan_repo(fixture.path())?;
    if !hits.is_empty() {
        return Err(Error::Process(format!(
            "self-test failed: {label} was a false positive"
        )));
    }
    println!("ok: {label} stayed clean");
    Ok(())
}

fn git_fixture(relative: &str, content: &str) -> Result<TempDir> {
    let fixture = tempfile::Builder::new()
        .prefix("blaksync-secret-")
        .tempdir()?;
    checked_git(fixture.path(), &["init", "-q"])?;
    checked_git(
        fixture.path(),
        &["config", "user.email", "ci@blaksync.example"],
    )?;
    checked_git(fixture.path(), &["config", "user.name", "BlakSync CI"])?;
    let destination = fixture.path().join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&destination, content)?;
    checked_git(fixture.path(), &["add", "-f", "--", relative])?;
    checked_git(fixture.path(), &["commit", "-qm", "test fixture"])?;
    Ok(fixture)
}

fn checked_git(root: &Path, args: &[&str]) -> Result<Output> {
    let output = git_output(root, args)?;
    if output.status.success() {
        return Ok(output);
    }
    Err(Error::Process(format!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn git_output(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sensitive_material_without_echoing_it() {
        let findings = findings_in("key.pem", &dummy_private_key());
        assert!(findings.iter().any(|item| item.rule == "private-key"));
        assert!(
            findings
                .iter()
                .any(|item| item.rule == "syncthing-device-key-file")
        );
        assert!(findings.iter().all(|item| !item.snippet.contains("MHcCAQ")));
    }

    #[test]
    fn allows_documentation_and_placeholders() {
        assert!(findings_in("README.md", "Never commit key.pem").is_empty());
        assert!(findings_in("config.xml", "<apikey>REDACTED</apikey>").is_empty());
        assert!(
            findings_in(
                "backend/config.rs",
                r#"extract_apikey("<gui><apikey>local-test-key-12345678901</apikey></gui>")"#
            )
            .is_empty()
        );
    }
}
