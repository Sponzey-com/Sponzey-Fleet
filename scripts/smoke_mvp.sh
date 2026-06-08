#!/usr/bin/env sh
set -eu

cargo build -p fleet-cli
BIN="./target/debug/sponzey"
WORK_DIR="${TMPDIR:-/tmp}/sponzey-fleet-smoke"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

INIT_OUTPUT="$("$BIN" controller init --data-dir "$WORK_DIR")"
printf '%s\n' "$INIT_OUTPUT"
ADMIN_TOKEN="$(printf '%s\n' "$INIT_OUTPUT" | sed -n 's/^admin token: //p')"
"./scripts/run_controller.sh" --host 127.0.0.1 --port 7700 --data-dir "$WORK_DIR" --dev-insecure-loopback > "$WORK_DIR/controller.log" 2>&1 &
CONTROLLER_PID="$!"
trap 'kill "$CONTROLLER_PID" 2>/dev/null || true' EXIT INT TERM

i=0
while [ "$i" -lt 50 ]; do
  if curl -fsS http://127.0.0.1:7700/healthz >/dev/null 2>&1; then
    break
  fi
  i=$((i + 1))
  sleep 0.1
done

if [ "$i" -eq 50 ]; then
  cat "$WORK_DIR/controller.log" >&2
  if grep -q "Operation not permitted (os error 1)" "$WORK_DIR/controller.log"; then
    echo "smoke skipped: loopback server bind is not permitted in this environment"
    exit 0
  fi
  echo "controller did not become healthy" >&2
  exit 1
fi

TOKEN="$("$BIN" enroll-token create --data-dir "$WORK_DIR" --labels role=web,env=dev)"
"$BIN" agent enroll --data-dir "$WORK_DIR" --url http://127.0.0.1:7700 --token "$TOKEN" --name web-01 --labels role=web,env=dev
"./scripts/run_agent.sh" --data-dir "$WORK_DIR" --dev-insecure-loopback --once
curl -fsS \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"job_id":"job-remote-1","target_agent_ids":[],"selector":"role=web","program":"printf","args":["remote-ok"],"timeout_seconds":30,"confirmed_high_risk":true,"confirmed_by":"smoke-admin","expires_in_seconds":60,"nonce_prefix":"remote-smoke"}' \
  http://127.0.0.1:7700/api/jobs/command >/dev/null
"./scripts/run_agent.sh" --data-dir "$WORK_DIR" --dev-insecure-loopback --once
REMOTE_OUTPUT="$(sqlite3 "$WORK_DIR/controller/fleet.db" "SELECT body FROM job_output_chunks WHERE job_id = 'job-remote-1' ORDER BY chunk_index")"
REMOTE_STATUS="$(sqlite3 "$WORK_DIR/controller/fleet.db" "SELECT status FROM jobs WHERE id = 'job-remote-1'")"
REMOTE_OUTPUT_API="$(curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" http://127.0.0.1:7700/api/jobs/job-remote-1/output)"
AGENTS_API="$(curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" http://127.0.0.1:7700/api/agents)"
FACTS_API="$(curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" http://127.0.0.1:7700/api/agents/agent-web-01/facts/latest)"
METRICS_API="$(curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" http://127.0.0.1:7700/api/agents/agent-web-01/metrics/latest)"
if [ "$REMOTE_OUTPUT" != "remote-ok" ] || [ "$REMOTE_STATUS" != "success" ]; then
  echo "remote command smoke failed: output=$REMOTE_OUTPUT status=$REMOTE_STATUS" >&2
  exit 1
fi
case "$REMOTE_OUTPUT_API" in
  *'"data":"remote-ok"'*) ;;
  *)
    echo "remote output API smoke failed: $REMOTE_OUTPUT_API" >&2
    exit 1
    ;;
esac
case "$AGENTS_API" in
  *'"id":"agent-web-01"'*'"status":"online"'*) ;;
  *'"id":"agent-web-01"'*'"status":"degraded"'*) ;;
  *)
    echo "agents API smoke failed: $AGENTS_API" >&2
    exit 1
    ;;
esac
case "$FACTS_API" in
  *'"agent_id":"agent-web-01"'*'"os"'*) ;;
  *)
    echo "facts API smoke failed: $FACTS_API" >&2
    exit 1
    ;;
esac
case "$METRICS_API" in
  *'"agent_id":"agent-web-01"'*'"cpu"'*) ;;
  *)
    echo "metrics API smoke failed: $METRICS_API" >&2
    exit 1
    ;;
esac
PATCH_LABELS_API="$(curl -fsS \
  -X PATCH \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"labels":[{"key":"role","value":"api"},{"key":"env","value":"dev"}]}' \
  http://127.0.0.1:7700/api/agents/agent-web-01/labels)"
case "$PATCH_LABELS_API" in
  *'"key":"role"'*'"value":"api"'*) ;;
  *)
    echo "agent label patch smoke failed: $PATCH_LABELS_API" >&2
    exit 1
    ;;
esac
"$BIN" agents list --data-dir "$WORK_DIR"
"$BIN" run --selector role=web --confirm-risk uptime
"$BIN" facts web-01
"$BIN" metrics web-01
"$BIN" drift check --policy examples/policies/nginx-running.yml
"$BIN" apply examples/runbooks/nginx-basic.yml
RUNBOOK_REQUEST="$WORK_DIR/runbook-request.json"
cat > "$RUNBOOK_REQUEST" <<'JSON'
{
  "job_id": "job-runbook-1",
  "target_agent_ids": [],
  "selector": "role=api",
  "runbook_document": "apiVersion: fleet.sponzey.dev/v1alpha1\nkind: Runbook\nmetadata:\n  name: nginx-basic\nspec:\n  targets:\n    selector: role=web\n  tasks:\n    - id: nginx-package\n      package:\n        name: nginx\n        state: present\n",
  "timeout_seconds": 30,
  "confirmed_high_risk": true,
  "confirmed_by": "smoke-admin",
  "expires_in_seconds": 60,
  "nonce_prefix": "runbook-smoke"
}
JSON
curl -fsS \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  --data-binary "@$RUNBOOK_REQUEST" \
  http://127.0.0.1:7700/api/jobs/runbook >/dev/null
"./scripts/run_agent.sh" --data-dir "$WORK_DIR" --dev-insecure-loopback --once
RUNBOOK_STATUS="$(sqlite3 "$WORK_DIR/controller/fleet.db" "SELECT status FROM jobs WHERE id = 'job-runbook-1'")"
RUNBOOK_OUTPUT="$(sqlite3 "$WORK_DIR/controller/fleet.db" "SELECT body FROM job_output_chunks WHERE job_id = 'job-runbook-1' ORDER BY chunk_index")"
case "$RUNBOOK_STATUS:$RUNBOOK_OUTPUT" in
  *"failed:"*"no supported Linux package manager detected"*) ;;
  *)
    echo "runbook signed dispatch smoke failed: status=$RUNBOOK_STATUS output=$RUNBOOK_OUTPUT" >&2
    exit 1
    ;;
esac
"$BIN" retention cleanup --data-dir "$WORK_DIR" --older-than-days 0 --dry-run

echo "smoke ok"
