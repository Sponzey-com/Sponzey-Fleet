# Sponzey Fleet Service Install Notes

Service installation supports dry-run rendering everywhere and guarded systemd writes on Linux when run as root. The service entrypoint always pins the resolved absolute Rust binary path instead of relying on an npm global shim.

## Commands

```bash
sponzey controller install-service --data-dir /var/lib/sponzey-fleet --dry-run
sponzey agent install-service --data-dir /var/lib/sponzey-fleet --dry-run
sponzey controller start-service --dry-run
sponzey agent start-service --dry-run
```

Without `--dry-run`, `install-service` writes `/etc/systemd/system/sponzey-fleet-controller.service` or `/etc/systemd/system/sponzey-fleet-agent.service`, then runs `systemctl daemon-reload` and `systemctl enable ...`. `start-service` runs `systemctl start ...`.

Non-Linux hosts fail with a clear Linux requirement. Linux hosts without root fail with a clear sudo/root requirement. Dry-run never writes system files.

The MVP repository also provides foreground scripts for local development:

```bash
./scripts/run_controller.sh
./scripts/run_agent.sh
```

`run_agent.sh` does not auto-enroll the agent. Use the same `--data-dir` for controller init, token creation, agent enroll, and agent start:

```bash
./target/debug/sponzey controller init --data-dir .sponzey
./scripts/run_controller.sh --host 127.0.0.1 --port 7700 --data-dir .sponzey --dev-insecure-loopback
TOKEN=$(./target/debug/sponzey enroll-token create --data-dir .sponzey --labels role=web,env=dev)
./target/debug/sponzey agent enroll --data-dir .sponzey --url http://127.0.0.1:7700 --token "$TOKEN" --name web-01 --labels role=web,env=dev
./scripts/run_agent.sh --data-dir .sponzey --dev-insecure-loopback
```

## Required Service Properties

Systemd unit generation:

- pin the resolved absolute Rust binary path,
- avoid relying on npm global shim paths for service execution,
- pass controller/agent role through explicit CLI arguments,
- pass data directory through explicit CLI arguments,
- avoid runtime environment mutation,
- fails clearly when Linux/root requirements are not met,
- supports dry-run output before writing system files.

## Manual Systemd Shape

Controller service direction:

```ini
[Unit]
Description=Sponzey Fleet Controller
After=network-online.target

[Service]
Type=simple
ExecStart=/absolute/path/to/sponzey controller start --data-dir /var/lib/sponzey-fleet
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Agent service direction:

```ini
[Unit]
Description=Sponzey Fleet Agent
After=network-online.target

[Service]
Type=simple
ExecStart=/absolute/path/to/sponzey agent start --data-dir /var/lib/sponzey-fleet
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Remote insecure transport must not be used in production. The loopback insecure flag is only for local development and demo flows.

## Manual Reboot Smoke

The repository includes a guarded manual smoke script for the destructive Linux/systemd verification that cannot run in the default local suite.

Requirements:

- Linux host
- root privileges
- systemd
- built `sponzey` binary or `SPONZEY_BIN` pointing to an absolute binary

Run before reboot:

```bash
sudo ./scripts/manual_systemd_reboot_smoke.sh install
# or run it through the release gate
sudo ./scripts/release_readiness_gate.sh --include-manual
```

Then reboot the host and verify:

```bash
sudo ./scripts/manual_systemd_reboot_smoke.sh verify
# or verify through the release gate
sudo ./scripts/release_readiness_gate.sh --verify-manual-reboot
```

The script checks that both `sponzey-fleet-controller.service` and `sponzey-fleet-agent.service` are enabled and active.

## Manual npm Registry Smoke

After publishing `@sponzey/fleet` and its platform packages to the npm registry:

```bash
./scripts/manual_npm_registry_smoke.sh
# or run it through the release gate
./scripts/release_readiness_gate.sh --include-registry
```

The script installs into a temporary npm prefix and verifies that `sponzey --help` runs through the installed wrapper.
