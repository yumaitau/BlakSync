#!/usr/bin/env python3
"""Scan a git repo for device keys and live config that must never be committed.

Equivalent to the extra rules in .gitleaks.toml so this tree can be checked
without calling a SaaS scanner. CI also runs gitleaks on the self-hosted runner.
Findings are redacted. Exit 1 if anything matches.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Split so this file never contains a full PEM header+body that a scanner
# would treat as a real key.
_BEGIN = "-----BEGIN"
_END = "-----END"
_PRIV = "PRIVATE KEY"
_CERT = "CERTIFICATE"
_DASH = "-" * 5

PEM_PRIVATE = re.compile(
    rf"{_BEGIN}[ A-Z0-9_-]{{0,100}}{_PRIV}(?: BLOCK)?{_DASH}[\s\S-]{{64,}}?KEY(?: BLOCK)?{_DASH}",
    re.IGNORECASE,
)
PEM_CERT = re.compile(
    rf"{_BEGIN}[ A-Z0-9_-]{{0,40}}{_CERT}{_DASH}",
    re.IGNORECASE,
)
APIKEY = re.compile(
    r"(?i)<apikey>\s*([A-Za-z0-9_-]{22,64})\s*</apikey>",
)
APIKEY_PLACEHOLDER = re.compile(
    r"(?i)^(REDACTED|YOUR[_-]?GUI[_-]?API[_-]?KEY|changeme|placeholder)$",
)
KEY_PEM_PATH = re.compile(r"(?i)(?:^|/)(?:https-)?key\.pem$")
CERT_PEM_PATH = re.compile(r"(?i)(?:^|/)(?:https-)?cert\.pem$")

SKIP_PATHS = (
    re.compile(r"(?i)(?:^|/)\.gitleaks\.toml$"),
    re.compile(r"(?i)(?:^|/)scripts/secret-scan\.py$"),
)


def redacted(text: str, limit: int = 24) -> str:
    compact = re.sub(r"\s+", " ", text).strip()
    if len(compact) <= limit:
        return compact[: max(4, limit // 2)] + "…"
    return compact[:12] + "…[redacted]…" + compact[-8:]


def skipped(path: str) -> bool:
    return any(p.search(path.replace("\\", "/")) for p in SKIP_PATHS)


def findings_in(path: str, content: str) -> list[tuple[str, str]]:
    if skipped(path):
        return []
    found: list[tuple[str, str]] = []
    posix = path.replace("\\", "/")
    if PEM_PRIVATE.search(content):
        found.append(("private-key", redacted(PEM_PRIVATE.search(content).group(0))))
    if KEY_PEM_PATH.search(posix) and _BEGIN in content:
        found.append(("syncthing-device-key-file", posix))
    if CERT_PEM_PATH.search(posix) and PEM_CERT.search(content):
        found.append(("syncthing-device-cert-file", posix))
    for match in APIKEY.finditer(content):
        value = match.group(1)
        if APIKEY_PLACEHOLDER.match(value):
            continue
        found.append(("syncthing-gui-apikey", "<apikey>…[redacted]…</apikey>"))
    return found


def git(args: list[str], cwd: Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True, stderr=subprocess.DEVNULL)


def tracked_paths(cwd: Path, rev: str) -> list[str]:
    out = git(["ls-tree", "-r", "--name-only", "-z", rev], cwd)
    return [p for p in out.split("\0") if p]


def blob_at(cwd: Path, rev: str, path: str) -> str | None:
    try:
        data = subprocess.check_output(
            ["git", "show", f"{rev}:{path}"],
            cwd=cwd,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return data.decode("utf-8", errors="replace")


def revisions(cwd: Path) -> list[str]:
    try:
        out = git(["rev-list", "--all"], cwd)
    except subprocess.CalledProcessError:
        return []
    return [line for line in out.splitlines() if line]


def scan_repo(cwd: Path) -> list[str]:
    lines: list[str] = []
    revs = revisions(cwd)
    if not revs:
        # Unborn or not a git repo: scan working tree files.
        for path in cwd.rglob("*"):
            if not path.is_file() or ".git" in path.parts:
                continue
            rel = str(path.relative_to(cwd))
            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            for rule, snippet in findings_in(rel, text):
                lines.append(f"{rel}: {rule}: {snippet}")
        return lines

    seen: set[tuple[str, str, str]] = set()
    for rev in revs:
        short = rev[:12]
        for path in tracked_paths(cwd, rev):
            content = blob_at(cwd, rev, path)
            if content is None:
                continue
            for rule, snippet in findings_in(path, content):
                key = (path, rule, snippet)
                if key in seen:
                    continue
                seen.add(key)
                lines.append(f"{short}:{path}: {rule}: {snippet}")
    return lines


def dummy_private_key() -> str:
    # Built at runtime. Body is long enough for the PEM rule; it is not a
    # real key and must never be used as one.
    body = (
        "MHcCAQEEIBlakSyncCiDummyKeyNotSecretAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
        "oAoGCCqGSM49AwEHoUQDQgAEDummyKeyForCiDetectionOnlyNotASecretXX\n"
        "BlakSyncIssue6DummyKeyPemMustFailTheScanXXXXXXXXXXXXXXXXXXXX==\n"
    )
    return f"{_BEGIN} EC {_PRIV}{_DASH}\n{body}{_END} EC {_PRIV}{_DASH}\n"


def dummy_apikey_config() -> str:
    # Assembled at runtime so this source file is not itself a finding.
    tag = "apikey"
    value = "k1dnz1Dd0rzTBjjFFh7" "CXPnrF12C49B1"
    return (
        '<configuration version="37">\n'
        "  <gui>\n"
        "    <address>127.0.0.1:8384</address>\n"
        f"    <{tag}>{value}</{tag}>\n"
        "  </gui>\n"
        "</configuration>\n"
    )


def init_git(path: Path) -> None:
    subprocess.check_call(["git", "init", "-q"], cwd=path)
    subprocess.check_call(["git", "config", "user.email", "ci@blaksync.example"], cwd=path)
    subprocess.check_call(["git", "config", "user.name", "BlakSync CI"], cwd=path)


def commit_file(path: Path, rel: str, content: str) -> None:
    dest = path / rel
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(content, encoding="utf-8")
    subprocess.check_call(["git", "add", "-f", "--", rel], cwd=path)
    subprocess.check_call(["git", "commit", "-qm", f"test: {rel}"], cwd=path)


def assert_detects(label: str, rel: str, content: str) -> None:
    with tempfile.TemporaryDirectory(prefix="blaksync-secret-") as tmp:
        root = Path(tmp)
        init_git(root)
        commit_file(root, rel, content)
        hits = scan_repo(root)
        if not hits:
            raise SystemExit(f"self-test failed: {label} was not detected")
        print(f"ok: {label} failed the scan ({len(hits)} finding(s))")


def assert_clean(label: str, rel: str, content: str) -> None:
    with tempfile.TemporaryDirectory(prefix="blaksync-secret-") as tmp:
        root = Path(tmp)
        init_git(root)
        commit_file(root, rel, content)
        hits = scan_repo(root)
        if hits:
            raise SystemExit(f"self-test failed: {label} was a false positive: {hits}")
        print(f"ok: {label} stayed clean")


def self_test() -> int:
    assert_detects("dummy key.pem on a test branch", "key.pem", dummy_private_key())
    assert_detects("dummy https-key.pem", "https-key.pem", dummy_private_key())
    assert_detects(
        "dummy cert.pem",
        "cert.pem",
        f"{_BEGIN} {_CERT}{_DASH}\nMIIBDummyCertNotADeviceIdentityXXXXXXXX\n",
    )
    assert_detects("live config.xml API key", "config.xml", dummy_apikey_config())
    assert_clean(
        "docs mentioning key.pem",
        "README.md",
        "Never commit cert.pem, key.pem, or config.xml with API keys.\n",
    )
    assert_clean(
        "placeholder apikey",
        "config.example.xml",
        "<apikey>REDACTED</apikey>\n",
    )
    print("self-test passed: dummy keys fail; docs do not")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        default=".",
        help="git repository to scan (default: current directory)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="commit a dummy key.pem in a throwaway repo and require a finding",
    )
    parser.add_argument(
        "--write-dummy-key",
        metavar="PATH",
        help="write a runtime dummy key.pem to PATH (for the gitleaks fixture)",
    )
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.write_dummy_key:
        Path(args.write_dummy_key).write_text(dummy_private_key(), encoding="utf-8")
        return 0

    cwd = Path(args.source).resolve()
    hits = scan_repo(cwd)
    if hits:
        print("secret scan failed (values redacted):", file=sys.stderr)
        for line in hits:
            print(f"  {line}", file=sys.stderr)
        return 1
    print(f"secret scan clean: {cwd}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as exc:
        print(f"secret-scan: git command failed: {exc}", file=sys.stderr)
        sys.exit(2)
