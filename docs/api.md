# Sponzey Fleet MVP API

이 문서는 현재 구현된 MVP Controller API 범위를 기록한다. API는 `sponzey controller start`로 실행되는 Controller process가 제공한다.

## Transport

MVP 개발 모드에서는 다음처럼 loopback에서만 insecure HTTP를 허용한다.

```bash
sponzey controller start --host 127.0.0.1 --port 7700 --data-dir .sponzey --dev-insecure-loopback
```

`--dev-insecure-loopback`은 `127.0.0.1`, `localhost`, `::1`에서만 허용한다. 원격 주소에서 insecure transport를 허용하지 않는다.

SQLite DB 경로를 명시하려면 bootstrap 시점에 `--db sqlite://...`를 전달한다.

```bash
sponzey controller start --host 127.0.0.1 --port 7700 --data-dir .sponzey --db sqlite:///tmp/sponzey-fleet.db --dev-insecure-loopback
```

## Health

```http
GET /healthz
```

응답:

```json
{"status":"ok"}
```

Health endpoint는 인증 없이 접근 가능하다. 이 endpoint는 process가 요청을 받을 수 있는지 확인하기 위한 최소 readiness surface다.

## Controller Identity

```http
GET /api/controller/identity
```

응답:

```json
{
  "controller_public_key": "<ed25519-public-key-hex>",
  "controller_fingerprint": "<sha256-public-key-fingerprint-hex>"
}
```

Agent는 시작 시 저장된 controller fingerprint와 이 응답을 비교한다. 값이 달라지면 explicit re-enroll 없이 연결하지 않는다.

## Admin Token

`sponzey controller init`은 최초 실행 시 admin token을 1회 출력한다.

```bash
sponzey controller init --data-dir .sponzey
```

출력 예:

```text
controller initialized at .sponzey
controller fingerprint: <sha256-public-key-fingerprint-hex>
admin token: admin-...
```

`controller init`은 controller Ed25519 key pair를 생성한다. public key는 `controller_public.key`, private key는 `controller_private.key`에 저장한다. Unix에서는 private key 파일이 group/other에 열려 있으면 init/start path에서 거부한다.

admin token 원문은 DB에 저장하지 않는다. Controller는 token hash만 저장한다. 이미 초기화된 controller에서 다시 `init`을 실행하면 controller key와 admin token을 새로 만들지 않는다.

MVP에서 Controller와 CLI가 새로 생성하는 token, job, assignment nonce, message id는 사람이 읽을 수 있는 prefix와 ULID를 조합한 형태를 사용한다. 예: `admin-...`, `enroll-...`, `et-...`, `job-cli-...`, `nonce-...`, `msg-...`.

보호 API는 다음 header를 요구한다.

```http
Authorization: Bearer <admin-token>
```

## Enrollment Token API

### Create

```http
POST /api/enrollment-tokens
Authorization: Bearer <admin-token>
```

응답은 raw enrollment token을 1회 포함한다.

```json
{"id":"et-...","token":"enroll-...","expires_in_seconds":3600}
```

raw enrollment token은 DB에 저장하지 않는다. DB에는 token hash와 metadata만 저장한다.
생성 이벤트는 enrollment audit에 남기되 raw token은 기록하지 않고 token id reference만 남긴다.

### List

```http
GET /api/enrollment-tokens
Authorization: Bearer <admin-token>
```

응답:

```json
[
  {
    "id": "et-...",
    "default_labels": "",
    "max_uses": 1,
    "used_count": 0,
    "revoked": false
  }
]
```

### Revoke

```http
DELETE /api/enrollment-tokens/{id}
Authorization: Bearer <admin-token>
```

성공 시 `204 No Content`를 반환한다.
폐기 이벤트도 enrollment audit에 남긴다.

## Agent Enrollment API

Agent는 enrollment token을 사용해 controller에 자기 identity를 등록한다. 이 endpoint는 admin token을 사용하지 않는다.

```http
POST /api/agents/enroll
Content-Type: application/json
```

요청:

```json
{
  "token": "enroll-...",
  "agent_id": "agent-web-01",
  "name": "web-01",
  "public_key": "<ed25519-public-key-hex>",
  "fingerprint": "<sha256-public-key-fingerprint-hex>",
  "labels": [
    {"key": "role", "value": "web"}
  ]
}
```

응답:

```json
{
  "agent_id": "agent-web-01",
  "controller_public_key": "<ed25519-public-key-hex>",
  "controller_fingerprint": "<sha256-public-key-fingerprint-hex>"
}
```

Controller는 raw enrollment token을 hash로 검증하고, 성공 시 token use count를 증가시킨다. 만료, 폐기, max uses 초과 token은 거부한다. 등록 시 public key와 fingerprint가 일치하지 않으면 거부한다. Enrollment token에 default labels가 있으면 agent labels에 적용하고, agent가 명시한 같은 key의 label은 default를 override한다.

## Command Job API

MVP의 command job API는 admin token 인증 후 command job과 controller-signed task assignment를 생성한다. 등록된 agent가 다음 heartbeat WebSocket session을 열면 controller는 queued assignment 하나를 dispatch하고, agent는 command를 실행한 뒤 output chunk와 task result를 controller에 돌려보낸다. 실행 중 실시간 streaming과 별도 output subscribe API는 아직 후속 범위다.

```http
POST /api/jobs/command
Authorization: Bearer <admin-token>
Content-Type: application/json
```

요청:

```json
{
  "job_id": "job-1",
  "target_agent_ids": ["agent-web-01"],
  "selector": null,
  "program": "uptime",
  "args": [],
  "timeout_seconds": 30,
  "confirmed_high_risk": true,
  "confirmed_by": "operator@example.com",
  "expires_in_seconds": 60,
  "nonce_prefix": "nonce-job-1"
}
```

응답:

```json
{
  "job_id": "job-1",
  "target_count": 1,
  "assignment_count": 1
}
```

`target_agent_ids`를 명시하면 해당 agent를 대상으로 한다. `target_agent_ids`가 비어 있고 `selector`가 있으면 controller가 inventory에서 matching agent를 찾아 target을 만든다. selector 예시는 `role=web,env=prod`, `agent:web-01`이다.

command task는 기본 high-risk로 분류된다. `confirmed_high_risk`가 `false`이면 job 생성 전에 거부한다. Controller는 private signing key로 task envelope signature를 만들고, job과 task assignment를 SQLite에 저장한 뒤 `job_created` audit event를 남긴다. `confirmed_by`는 high-risk 확인 actor로 audit에 남긴다. Dispatch 시 `job_started`, result 수신 시 `job_completed` 또는 `job_failed` audit event도 남긴다.

CLI에서 controller API로 job을 생성하려면 admin token을 명시 인자로 전달한다. token은 command payload나 job output에 섞지 않는다.

```bash
sponzey run \
  --controller-url http://127.0.0.1:7700 \
  --admin-token <admin-token> \
  --selector role=web \
  --confirm-risk \
  uptime
```

## Runbook Job API

Runbook job API는 admin token 인증 후 runbook 문서를 validation하고, controller-signed task assignment를 생성한다. 실제 package/service/file primitive 실행은 agent가 signed envelope를 검증한 뒤 수행한다. `sponzey apply`는 local validation-only 명령이며, privileged execution path가 아니다.

```http
POST /api/jobs/runbook
Authorization: Bearer <admin-token>
Content-Type: application/json
```

요청:

```json
{
  "job_id": "job-nginx-runbook-1",
  "target_agent_ids": [],
  "selector": "role=web",
  "runbook_document": "apiVersion: fleet.sponzey.dev/v1alpha1\nkind: Runbook\nmetadata:\n  name: nginx-basic\nspec:\n  targets:\n    selector: role=web\n  tasks:\n    - id: nginx-package\n      package:\n        name: nginx\n        state: present\n",
  "timeout_seconds": 180,
  "confirmed_high_risk": true,
  "confirmed_by": "operator@example.com",
  "expires_in_seconds": 300,
  "nonce_prefix": "nonce-nginx-runbook"
}
```

응답:

```json
{
  "job_id": "job-nginx-runbook-1",
  "target_count": 1,
  "assignment_count": 1
}
```

규칙:

- invalid runbook은 task assignment 생성 전에 거부한다.
- runbook execution task는 high-risk로 분류한다.
- `confirmed_high_risk`가 `false`이면 거부한다.
- selector resolution은 disabled agent를 제외한다.
- agent는 signed envelope 검증, expiry 검증, replay 검증 이후에만 실행한다.
- step output은 job output chunk로 저장하고 Product application log와 분리한다.

### List Jobs

Web Admin UI와 CLI 확인용으로 최근 job summary를 조회한다.

```http
GET /api/jobs
Authorization: Bearer <admin-token>
```

응답:

```json
[
  {
    "id": "job-1",
    "status": "queued",
    "risk": "high",
    "command_program": "uptime",
    "command_args": ["-a"],
    "target_count": 1,
    "created_at_ms": 1710000000000
  }
]
```

이 API는 저장된 summary만 보여준다. authorization 판단이나 job 상태 전이는 controller application/domain 경계에서 처리한다.

### Poll Output

MVP는 실시간 subscribe 대신 polling 방식 output 조회 API를 제공한다.

```http
GET /api/jobs/{job_id}/output
Authorization: Bearer <admin-token>
```

응답:

```json
[
  {
    "job_id": "job-1",
    "agent_id": "agent-web-01",
    "stream": "stdout",
    "sequence": 0,
    "data": "ok"
  }
]
```

이 API는 job output storage를 조회한다. command stdout/stderr는 Product application log에 자동 기록하지 않는다.

## Agent Inventory API

### List

```http
GET /api/agents
Authorization: Bearer <admin-token>
```

응답:

```json
[
  {
    "id": "agent-web-01",
    "name": "web-01",
    "status": "online",
    "fingerprint": "<agent-fingerprint>",
    "labels": [
      {"key": "role", "value": "web"}
    ],
    "last_seen_at_ms": 1710000000000
  }
]
```

### Detail

```http
GET /api/agents/{agent_id}
Authorization: Bearer <admin-token>
```

응답은 list item과 같은 shape의 단일 object다. 존재하지 않는 agent는 `404`를 반환한다. Agent public key 원문은 이 API에 노출하지 않는다.

### Update Labels

```http
PATCH /api/agents/{agent_id}/labels
Authorization: Bearer <admin-token>
Content-Type: application/json
```

요청:

```json
{
  "labels": [
    {"key": "role", "value": "api"},
    {"key": "env", "value": "prod"}
  ]
}
```

응답은 갱신된 agent detail object다. Label key/value는 domain validation을 통과해야 한다. 성공 시 `agent_labels_updated` audit event를 남기며, audit에는 label 원문 전체 대신 label count 중심 metadata를 기록한다.

### Latest Facts

```http
GET /api/agents/{agent_id}/facts/latest
Authorization: Bearer <admin-token>
```

응답:

```json
{
  "agent_id": "agent-web-01",
  "collected_at_ms": 1710000000000,
  "body": {
    "os": "linux",
    "arch": "x86_64",
    "family": "unix",
    "cpu": {
      "logical_count": 4
    },
    "memory": {
      "total_kb": 16384256,
      "available_kb": 8123456
    },
    "network": {
      "interfaces": ["lo", "eth0"]
    },
    "disk": {
      "root_mount_known": true,
      "usage_available": true,
      "total_kb": 52428800,
      "used_kb": 18432000,
      "available_kb": 33996800,
      "used_percent": 35
    },
    "degraded": {
      "status": false,
      "signals": []
    }
  }
}
```

MVP의 agent facts snapshot은 heartbeat session에서 전송된다. 현재 수집 범위는 OS, architecture, platform family, hostname, CPU logical count, Linux `/proc/meminfo` 기반 memory, Linux `/proc/net/dev` 기반 network interface, root disk usage다. Facts payload의 `degraded.status=true`는 controller에서 agent 상태 `degraded`로 반영된다.

### Latest Metrics

```http
GET /api/agents/{agent_id}/metrics/latest
Authorization: Bearer <admin-token>
```

응답:

```json
{
  "agent_id": "agent-web-01",
  "collected_at_ms": 1710000000000,
  "body": {
    "cpu": {
      "logical_count": 4
    },
    "memory": {
      "total_kb": 16384256,
      "available_kb": 8123456
    },
    "process": {
      "pid": 1234,
      "count": 92
    },
    "service": {
      "status_available": true,
      "failed_units_count": 0,
      "failed_units": []
    },
    "disk": {
      "usage_available": true,
      "total_kb": 52428800,
      "used_kb": 18432000,
      "available_kb": 33996800,
      "used_percent": 35
    }
  }
}
```

Metrics snapshot도 heartbeat session에서 전송된다. MVP는 lightweight snapshot만 저장하며 time-series observability platform으로 확장하지 않는다. `service.status_available=false`는 systemd가 없거나 조회가 불가능한 환경을 의미하며, collector 실패로 process를 중단하지 않는다. Retention cleanup은 `sponzey retention cleanup`으로 명시적으로 실행한다.

### Latest Drift Report

```http
GET /api/agents/{agent_id}/drift/latest
Authorization: Bearer <admin-token>
```

응답:

```json
{
  "agent_id": "agent-web-01",
  "checked_at_ms": 1710000000000,
  "policy_name": "nginx-running",
  "status": "drifted",
  "expected": "service nginx running",
  "actual": "stopped"
}
```

Agent가 WebSocket task-data channel로 보낸 drift report는 `drift_reports`에 저장되고 `drift_report_received` audit event를 남긴다. Local `sponzey drift check --policy`는 service running, package present, file SHA-256 check engine을 사용한다. 다만 controller가 signed drift job을 agent에 dispatch하는 흐름은 아직 후속 범위다.

### Audit Events

```http
GET /api/audit
Authorization: Bearer <admin-token>
```

응답:

```json
[
  {
    "category": "security",
    "action": "invalid_signature",
    "actor": "system",
    "target": "agent-web-01",
    "value_kind": "redacted",
    "value": "redacted",
    "occurred_at_ms": 1710000000000
  }
]
```

Audit API는 최근 50개 event를 최신순으로 반환한다. `SecretRef` 값은 원문을 반환하지 않고 `secret_ref` marker로만 노출한다.

## Current Limits

- Axum/Actix 같은 production-grade HTTP framework로 아직 전환하지 않았다.
- Controller accept loop는 명시적 shutdown signal 경계를 갖지만, process signal integration은 CLI/runtime 후속 작업이다.
- controller key pair rotation은 아직 구현하지 않았다.
- admin token CLI profile 저장 방식은 아직 구현하지 않았다.
- WebSocket heartbeat 이후 queued command assignment dispatch와 completed output/result 수신은 동작한다. Web Admin UI는 command job 생성과 polling 기반 output viewer를 제공한다. CLI live renderer, true streaming subscribe, multi-agent fan-out 상태 집계는 후속 task 범위다.
