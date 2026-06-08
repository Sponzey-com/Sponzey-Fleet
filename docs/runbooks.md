# Sponzey Fleet MVP Runbooks

MVP runbooks are not Ansible playbooks. They are a small Sponzey-specific YAML surface for validating intended package, service, and file-copy primitives before those primitives are executed through signed task dispatch.

## Validate

```bash
sponzey apply examples/runbooks/nginx-basic.yml
```

Current `apply` behavior remains validation-only:

- parses the runbook,
- validates required fields,
- rejects unsupported top-level fields,
- rejects unsupported task kinds,
- rejects unsafe `file.copy.dest` paths,
- lowers supported tasks into a primitive execution plan,
- does not execute package, service, or file changes directly.

Execution must remain behind controller-signed task envelopes, high-risk confirmation, and audit. The MVP controller exposes that execution path through `POST /api/jobs/runbook`; the CLI `apply` command does not run privileged changes locally.

## Primitive Readiness

The runner layer now has command-builder primitives for package/service operations, a guarded `file.copy` primitive, and a runbook execution-plan builder. The plan builder maps:

- `package state: present` to package-present check plus package-install command steps,
- `service state: started|restarted` to systemd status plus apply command steps,
- `service enabled: true` to an explicit systemd enable command step,
- `file.copy` to a guarded file-copy spec.

The file-copy primitive is intentionally narrow:

- destination must be absolute and must not traverse through `..`,
- parent directory must already exist,
- content is written through a same-directory temporary file followed by rename,
- unchanged content returns `changed=false`,
- before/after SHA-256 checksums are returned,
- audit metadata includes destination, changed flag, and byte count,
- owner/group management is out of MVP scope and must be modeled as a later explicit primitive.

This does not change `sponzey apply`: local apply output is a validation aid, not local privileged execution. Actual package/service/file changes are executed by an enrolled agent after controller-signed dispatch.

## Signed Runbook Dispatch

The controller accepts a runbook job request at `POST /api/jobs/runbook`.

Minimal request shape:

```json
{
  "job_id": "job-nginx-runbook-1",
  "target_agent_ids": [],
  "selector": "role=web",
  "runbook_document": "apiVersion: fleet.sponzey.dev/v1alpha1\nkind: Runbook\nmetadata:\n  name: nginx-basic\nspec:\n  targets:\n    selector: role=web\n  tasks:\n    - id: nginx-package\n      package:\n        name: nginx\n        state: present\n",
  "timeout_seconds": 180,
  "confirmed_high_risk": true,
  "confirmed_by": "operator",
  "expires_in_seconds": 300
}
```

Rules:

- invalid runbooks are rejected before task assignment,
- high-risk confirmation is required,
- selector resolution excludes disabled agents,
- the agent verifies the signed envelope before executing,
- runbook step output is stored as job output chunks,
- app logs do not receive raw command output.

## Minimal Schema

```yaml
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Runbook
metadata:
  name: nginx-basic
spec:
  targets:
    selector: role=web
  tasks:
    - id: nginx-package
      package:
        name: nginx
        state: present
    - id: nginx-service
      service:
        name: nginx
        state: started
        enabled: true
```

Supported task declarations in the parser:

- `package` with `name` and `state: present`
- `service` with `name`, `state: started|restarted`, optional `enabled: true|false`
- `file.copy` with absolute safe `dest`, inline `content`, optional `mode`

Unsupported YAML constructs, unknown top-level fields, and Ansible compatibility assumptions are intentionally rejected or left out of scope.

## Manual Linux Nginx Smoke

The repository includes an ignored runner integration test and a signed-dispatch wrapper script for the destructive Linux check that cannot run in the default macOS or CI path.

Requirements:

- Linux host
- root privileges
- systemd
- `apt-get`, `dnf`, `yum`, or `apk`

Run:

```bash
sudo ./scripts/manual_linux_nginx_runbook_smoke.sh
# or run it through the release gate
sudo ./scripts/release_readiness_gate.sh --include-manual
```

The script starts a local controller, enrolls a local agent, creates a signed runbook job through `POST /api/jobs/runbook`, runs the agent once, installs nginx when missing, enables/starts `nginx.service`, and verifies that the service is active.
