# Contributing to BlakSync

## Agents and secrets

Agents (including Cursor and other automated helpers) must not commit secrets.

Do not add or stage:

- `.env` files or other environment dumps with real values
- Private keys (`*.pem`, `*.key`, `id_rsa`, `id_ed25519`, and similar)
- API tokens, passwords, or certificate material
- Anything under local `secrets/` or `keys/` directories

Use `.env.example` (values empty or placeholders only) when documenting required variables. Prefer GitHub Actions secrets or the org's own secret store for CI and deployment credentials.

If a secret is committed by mistake, rotate it immediately and remove it from history with maintainer help. Do not rely on a follow-up commit alone.
