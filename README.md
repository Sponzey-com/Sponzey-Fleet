# Sponzey Fleet

[한국어 문서](README.ko.md)

Sponzey Fleet is an agent-based fleet operations platform. It is designed for teams that need real-time server automation, state collection, drift checks, command execution, audit logs, and a lightweight Web Admin UI without relying on inbound SSH access.

The core runtime is Rust. The project is split into focused crates internally, but the product is distributed as one `sponzey` binary:

```text
sponzey controller ...
sponzey agent ...
sponzey enroll-token ...
sponzey run ...
sponzey demo
```

The npm package is a distribution wrapper for the Rust binary. It is not the runtime architecture.

## What It Does

Sponzey Fleet currently focuses on the MVP control loop:

- initialize a local controller with a one-time admin token,
- create enrollment tokens,
- enroll an outbound agent,
- pin the controller identity on the agent,
- receive authenticated WebSocket heartbeats,
- dispatch controller-signed tasks,
- collect command output, facts, metrics, drift reports, jobs, and audit events,
- serve a lightweight Web Admin UI from the controller at `/admin`.

This is not a full Ansible-compatible system. The product direction is to take the best parts of Ansible-like automation, agent-based fleet tools, runner systems, and audited runbook platforms while keeping the first product small, testable, and secure by default.

## Quick Story: Initialize One Agent And Open The Web UI

This story assumes the npm package is already installed. Installation is intentionally skipped here.

```bash
npm install -g @sponzey/fleet
sponzey --help
```

For local source development, build the Rust CLI and replace `sponzey` in the examples with `./target/debug/sponzey`:

```bash
cargo build -p fleet-cli
./target/debug/sponzey --help
```

### 1. Start A Local Controller

Use a local data directory for the demo:

```bash
sponzey controller init --data-dir .sponzey
```

`controller init` prints a one-time admin token. Copy it. The token is needed when you open the Web Admin UI or call protected APIs.

Start the controller in a separate terminal:

```bash
sponzey controller start \
  --host 127.0.0.1 \
  --port 7700 \
  --data-dir .sponzey \
  --external-url http://127.0.0.1:7700 \
  --dev-insecure-loopback
```

`--dev-insecure-loopback` is only for local `127.0.0.1` development. Do not use it for remote or production deployments.

### 2. Initialize The Agent By Enrolling It

In Sponzey Fleet, an agent is initialized by enrollment. The controller creates a short-lived enrollment token, and the agent stores its local identity and pinned controller fingerprint in the same data directory.

```bash
TOKEN=$(sponzey enroll-token create \
  --data-dir .sponzey \
  --labels role=web,env=dev)

sponzey agent init \
  --data-dir .sponzey \
  --url http://127.0.0.1:7700 \
  --token "$TOKEN" \
  --name web-01 \
  --labels role=web,env=dev
```

`sponzey agent init` is the first-time setup command. `sponzey agent enroll` remains available as a compatibility alias when you want to describe the controller enrollment flow explicitly.

Start the agent once to send a heartbeat, facts, and metrics, then exit:

```bash
sponzey agent start \
  --data-dir .sponzey \
  --dev-insecure-loopback \
  --once
```

For a foreground agent loop, omit `--once`:

```bash
sponzey agent start \
  --data-dir .sponzey \
  --dev-insecure-loopback
```

Local development scripts wrap the same single binary:

```bash
./scripts/run_controller.sh --host 127.0.0.1 --port 7700 --data-dir .sponzey --external-url http://127.0.0.1:7700 --dev-insecure-loopback
./scripts/run_agent.sh --data-dir .sponzey --dev-insecure-loopback
```

### Remote Laptop Development With SSH Tunnel

For another laptop, do not use a LAN `http://192.168.x.x:7700` controller URL. The MVP intentionally rejects non-loopback insecure HTTP. Keep the controller bound to loopback on the controller laptop, then expose it to the agent laptop with SSH port forwarding.

On the controller laptop:

```bash
sponzey controller init --data-dir .sponzey

sponzey controller start \
  --host 127.0.0.1 \
  --port 7700 \
  --data-dir .sponzey \
  --external-url http://127.0.0.1:7700 \
  --dev-insecure-loopback
```

Create the enrollment token on the controller laptop and copy only that token to the agent laptop:

```bash
sponzey enroll-token create --data-dir .sponzey --labels role=web,env=dev
```

On the agent laptop, open the tunnel and keep it running:

```bash
ssh -N -L 7700:127.0.0.1:7700 <user>@<controller-laptop-hostname-or-ip>
```

In another terminal on the agent laptop, initialize and start the agent through the tunnel:

```bash
sponzey agent init \
  --data-dir .sponzey \
  --url http://127.0.0.1:7700 \
  --token "<enrollment-token-from-controller-laptop>" \
  --name laptop-02 \
  --labels role=dev-laptop,env=dev

sponzey agent start \
  --data-dir .sponzey \
  --dev-insecure-loopback
```

The tunnel must stay open while the agent is running. For production remote enrollment and agent traffic, use HTTPS/TLS rather than insecure HTTP.

### 3. View The Agent In The Web Admin UI

Open:

```text
http://127.0.0.1:7700/admin
```

Paste the one-time admin token printed by `sponzey controller init`.

The MVP Web Admin UI is intentionally lightweight. It is served by the controller and does not require a separate Node.js web server. Use it to inspect:

- enrolled agents and labels,
- latest facts and metrics,
- latest drift result,
- command job creation with explicit high-risk confirmation,
- job output,
- recent jobs,
- audit events.

### 4. Remove The Agent

Current MVP cleanup removes the local agent identity and configuration. Stop the agent process first, then remove the local agent directory:

```bash
rm -rf .sponzey/agent
```

The controller-side inventory and audit records are intentionally retained for traceability. To use the same host again, create a new enrollment token and run `sponzey agent init` again.

A production-ready delete or disable flow should be audited and controller-side, for example:

```bash
sponzey agents disable <agent-id>
sponzey agents delete <agent-id> --confirm
```

Those audited controller-side delete commands are product direction, not the current MVP command surface.

## One-Command Local Demo

For a quick local product feel:

```bash
sponzey demo
```

The demo starts a local controller, enrolls a local agent, runs a sample job, and prints the Web Admin URL.

## Help

Use command help to inspect the available surface:

```bash
sponzey --help
sponzey controller --help
sponzey controller start --help
sponzey agent --help
sponzey agent init --help
sponzey agent start --help
```

The local wrapper scripts are role-specific:

```bash
./scripts/run_controller.sh --help
./scripts/run_agent.sh --help
```

## Safety Model

Sponzey Fleet is a remote operations platform and can eventually run as root on managed hosts. The MVP therefore keeps strict boundaries:

- agents connect outbound to the controller,
- enrollment tokens are one-time registration inputs,
- agents pin the controller signing fingerprint,
- TLS certificate trust protects transport, while the controller signing fingerprint protects product identity,
- replacing a TLS certificate is allowed when the pinned controller signing fingerprint stays the same,
- changing the controller signing key requires explicit agent re-enrollment or a future audited rotation flow,
- task envelopes are signed by the controller,
- unsigned, expired, replayed, or target-mismatched tasks are rejected by the agent,
- high-risk commands require explicit confirmation,
- command output is stored separately from application logs,
- secrets are redacted from logs and audit-oriented surfaces,
- non-loopback insecure transport is rejected.

## Development Verification

Run the local release gate before treating a change as ready:

```bash
./scripts/release_readiness_gate.sh
```

Useful narrower checks:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
npm test --workspace web-admin
npm run build --workspace web-admin
./scripts/smoke_mvp.sh
```

## Project Direction

The current MVP is intentionally small. The next product steps are:

- production TLS deployment,
- audited agent disable/delete,
- stronger service installation paths,
- controller-side retention workers,
- richer runbook execution,
- production key rotation,
- a generated API client for the Web Admin UI,
- packaged releases through npm, standalone binaries, and OS packages.
