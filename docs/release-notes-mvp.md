# Sponzey Fleet MVP Release Notes

This document captures the current MVP state and known limits.

## Included

- Rust workspace with layered crate boundaries.
- Single Rust `sponzey` CLI binary.
- Controller initialization with Ed25519 identity and one-time admin token output.
- Enrollment token create/list/revoke API.
- Agent enrollment with controller fingerprint pinning.
- Authenticated outbound WebSocket heartbeat.
- Controller-signed command task envelope.
- Agent-side signature, expiry, replay nonce, and target validation.
- High-risk command confirmation boundary.
- Command output storage separated from application logs.
- Minimal facts and metrics snapshot storage and API.
- Agent inventory and label update API.
- Controller-served static `/admin` placeholder UI.
- MVP runbook parser, `sponzey apply` validation-only command, and signed controller-to-agent runbook dispatch API.
- Explicit retention cleanup command for bounded job output, facts, and metrics storage.
- Local file log tail with redaction, follow mode, max-duration guard, and journald shortcut skeleton.
- Local policy drift check engine for service running, package present, and file SHA-256 checks with signed drift job dispatch.
- Web Admin UI can select an agent, create a confirmed high-risk command job, and view polling-based job output.
- npm package wrapper for Rust binary distribution.
- `sponzey demo` local loopback demo through the npm wrapper.
- Local MVP smoke script.
- Hardening audit script.

## Known Limits

- Controller HTTP/WebSocket serving uses Axum.
- No TLS production deployment path yet.
- No admin token CLI profile storage yet.
- Agent command execution streams output chunks before process completion; Web Admin UI uses polling/storage.
- Web Admin UI covers agent inventory, command job creation, output, facts, metrics, drift, jobs, and audit.
- No Ansible compatibility layer.
- `sponzey apply` validates only. Package/service/file primitive execution requires controller-signed runbook dispatch, high-risk confirmation, and an enrolled agent.
- Systemd install/start commands are implemented for Linux root environments; reboot verification remains manual.
- No background retention cleanup worker yet.
- No production key rotation flow yet.

## Demo Safety

HTTP controller URLs are allowed for setup checks, local development, lab tests, and short-lived validation only. Product, customer, production, shared, or long-running environments must use HTTPS.

Every HTTP path prints an insecure transport warning because traffic is not encrypted. A controller configured with an HTTP external URL also writes a Security audit event. HTTP transport provides no confidentiality or integrity guarantee and can expose tokens, commands, operational data, and traffic to man-in-the-middle attacks.

## Verification

Current MVP readiness is checked with:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
npm test --prefix npm/fleet
npm run build --workspace web-admin
npm test --workspace web-admin
./scripts/npm_local_pack_smoke.sh
./scripts/npm_platform_local_install_smoke.sh
./scripts/npm_demo_smoke.sh
./scripts/smoke_mvp.sh
./scripts/hardening_audit.sh
```

For a full local release gate:

```bash
./scripts/release_readiness_gate.sh
```

For destructive Linux checks, run on a Linux host with root privileges:

```bash
sudo ./scripts/release_readiness_gate.sh --include-manual
sudo reboot
sudo ./scripts/release_readiness_gate.sh --verify-manual-reboot
```

After npm registry publish, verify the installed wrapper with:

```bash
./scripts/release_readiness_gate.sh --include-registry
```
