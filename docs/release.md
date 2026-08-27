# Release SOP

Cut a production version from this page alone.

## Version source of truth

- Rust crate: `Cargo.toml` `version`
- GUI: `package.json` `version`

They must match. CI fails if they drift (`scripts/check-versions.sh`).

## Cut v0.2.0 (example)

1. Create a branch from `main`.
2. Set both version files to `0.2.0`.
3. Move the `[Unreleased]` notes in `CHANGELOG.md` under `## [0.2.0] — YYYY-MM-DD`. Use Australian English. List user-facing changes, not a commit dump.
4. Run the usual checks in `CONTRIBUTING.md`.
5. Open a pull request. Wait for self-hosted CI.
6. Merge.
7. Create a signed tag:

```sh
git tag -s v0.2.0 -m "BlakSync 0.2.0"
git push origin v0.2.0
```

8. The release workflow publishes artefacts and `SHA256SUMS` when secrets are present on the self-hosted runner. Signing keys never go in git.

## Tag message

Keep it short: the version and one sentence a person can read aloud.

## Honest limits

This SOP does not list the app on a store. It does not silently replace binaries on field laptops.
