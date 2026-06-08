# Sponzey Fleet MVP 구현 계획

작성일: 2026-06-04  
대상 문서: `PROJECT.md`, `AGENTS.md`, `RESEARCH.md`  
목표: Rust core 기반 Controller/Agent/CLI와 가벼운 Web Admin UI로 MVP를 구현한다. npm 설치 UX는 유지하되, npm은 Rust 바이너리 배포 wrapper로 사용한다.

## 0. MVP 정의

MVP는 다음 경험을 끝까지 제공해야 한다.

```text
npm install -g @sponzey/fleet
sponzey controller init
sponzey controller start
sponzey enroll-token create
sponzey agent enroll --url http://127.0.0.1:7700 --token <token>
sponzey agent start
sponzey run --selector role=web "uptime"
sponzey facts web-01
sponzey metrics web-01
sponzey logs web-01 --file /var/log/syslog
sponzey drift check --policy nginx-running
```

MVP 성공 기준:

- 로컬에서 10분 안에 controller와 agent 1개를 실행할 수 있다.
- Linux 서버 3대를 30분 안에 등록할 수 있다.
- agent는 controller로 outbound WebSocket 연결을 유지한다.
- loopback demo를 제외한 agent-controller 통신은 TLS를 사용한다.
- agent는 enrollment 이후 controller identity를 pinning한다.
- controller는 agent identity를 검증한 뒤에만 heartbeat/task channel을 허용한다.
- agent는 controller-signed task envelope만 실행한다.
- controller는 agent heartbeat와 last seen을 저장한다.
- CLI에서 label selector로 agent를 대상으로 선택한다.
- command task 실행 결과가 stdout/stderr stream으로 표시된다.
- job history, job result, audit event가 SQLite에 저장된다.
- 기본 facts와 metrics snapshot을 볼 수 있다.
- 파일 로그 tail을 stream으로 볼 수 있다.
- nginx service running 정책의 drift를 감지할 수 있다.
- Web Admin UI에서 agent, job, output, facts/metrics, drift, audit을 확인할 수 있다.

MVP 제외:

- HA controller
- multi-tenant
- Windows/macOS agent service
- SAML/LDAP
- Vault/OpenBao/CyberArk integration
- Ansible full import
- plugin marketplace
- Kubernetes deployment
- full time-series metrics database
- auto remediation without approval
- production-grade mTLS automation

MVP에서 제외하지 않는 보안 바닥선:

- controller signing key 생성과 저장
- agent key pair 생성과 저장
- enrollment 시 controller public key pinning
- WebSocket 연결 시 agent identity proof
- task envelope 서명과 agent-side verification
- loopback 전용 insecure dev mode

## 1. 전체 개발 원칙

### 1.1 반드시 지킬 원칙

모든 작업은 `AGENTS.md`의 최상위 기초룰을 따른다.

- Layered Architecture
- Clean Architecture
- Tidy First
- TDD
- 외부 설정 파일 최소화
- 프로세스 중간 환경 설정 변경 금지
- 외부 환경 상수는 bootstrap 시점에 1회만 수집
- 로그는 Product, Field Debug, Development/Test 3단계로 분리

### 1.2 코드 계층

MVP crate 구조:

```text
crates/
  fleet-domain/
  fleet-application/
  fleet-protocol/
  fleet-store/
  fleet-runner/
  fleet-controller/
  fleet-agent/
  fleet-cli/
web-admin/
npm/
docs/
```

의존 방향:

```text
fleet-domain
  no outer dependency

fleet-application
  -> fleet-domain

fleet-protocol
  -> fleet-domain where shared types are intentional

fleet-store
  -> fleet-domain
  -> fleet-application contracts

fleet-runner
  -> fleet-domain

fleet-controller
  -> fleet-application
  -> fleet-store
  -> fleet-protocol

fleet-agent
  -> fleet-protocol
  -> fleet-runner
  -> fleet-domain

fleet-cli
  -> generated/client protocol types
```

금지:

- domain layer에서 DB, HTTP, WebSocket, filesystem, env var 접근
- application layer에서 `std::env` 직접 접근
- handler에 business logic 작성
- runtime 중 `std::env::set_var`로 설정 변경
- Product 로그에 command output 원문 기록

아키텍처 검증:

- workspace dependency policy를 문서화한다.
- `fleet-domain`은 framework, async runtime, DB, HTTP dependency를 갖지 않는다.
- `fleet-application`은 concrete infrastructure type을 import하지 않는다.
- protocol type은 한 곳에서 정의하고 API/UI/client에 생성 또는 변환으로 전달한다.
- CI에는 최소한 `cargo tree` 또는 전용 script로 금지 dependency를 확인하는 architecture fitness check를 둔다.

### 1.3 TDD 기준

각 기능 작업은 기본적으로 다음 순서를 따른다.

1. domain/application test 먼저 작성
2. 최소 구현
3. repository/transport integration test 추가
4. CLI/API 연결
5. Web Admin UI 연결
6. smoke test로 end-to-end 확인

문서와 scaffolding은 테스트 없이 가능하지만, 다음 영역은 테스트 없이 구현하지 않는다.

- enrollment token
- agent identity
- signed task 준비 구조
- job state machine
- command dispatch
- audit event 생성
- log redaction
- settings parsing
- selector matching
- drift detection

### 1.4 MVP 보안 바닥선

MVP는 실험용이어도 원격 명령 실행 제품이다. 다음은 hardening 단계로 미루지 않는다.

Identity:

- controller는 `controller init` 시 signing key pair를 만든다.
- agent는 enrollment 시 agent key pair를 만든다.
- controller는 agent public key/fingerprint를 저장한다.
- agent는 controller public key를 local config에 pinning한다.

Transport:

- loopback demo는 `http://127.0.0.1`을 허용한다.
- non-loopback URL은 TLS를 요구한다.
- insecure mode는 `--dev-insecure-loopback` 같은 명시적 이름을 사용하고 Product 로그에 남긴다.
- insecure mode는 원격 주소에서 거부한다.

Task integrity:

- controller는 task assignment를 signed envelope로 보낸다.
- envelope에는 job id, task id, target agent id, issued at, expires at, nonce, payload hash가 들어간다.
- agent는 서명, target agent id, expiry, nonce replay를 검증한 뒤 실행한다.
- 검증 실패는 security audit event로 남긴다.

High-risk execution:

- `command`, `shell`, `reboot`, `service.restart`, package remove, destructive file operation은 high risk다.
- MVP에서 full approval workflow가 없어도 high-risk task는 admin token과 명시적 confirmation flag를 요구한다.
- Web Admin UI는 high-risk action에 confirmation UI를 반드시 둔다.
- high-risk auto execution은 허용하지 않는다.

### 1.5 로그 프로파일

MVP에서 구현할 로그 프로파일:

```text
Product
  기본값
  최소 운영 로그

FieldDebug
  현장 진단용
  secret redaction 필수

Development
  로컬 개발/테스트용
  test fixture 중심
```

로그 프로파일 선택:

- bootstrap 시점에 CLI arg 또는 최초 settings로 결정한다.
- 실행 중 env var나 UI로 global log profile을 변경하지 않는다.
- 변경은 restart 또는 명시적 diagnostic session으로만 설계한다.

## 2. 마일스톤 개요

| Milestone | 목표                    | 핵심 산출물                                   |
| --------- | --------------------- | ---------------------------------------- |
| M0        | 프로젝트 골격               | Rust workspace, 기본 CLI, settings/logging |
| M1        | Domain/Application 기반 | agent/job/audit/runbook/selector 모델      |
| M2        | Storage 기반            | SQLite migration, repository contract    |
| M3        | Controller API        | REST API, admin token, enrollment token  |
| M4        | Agent 연결              | outbound WebSocket, heartbeat, last seen |
| M5        | Remote Run            | command task, live output, job history   |
| M6        | Inventory/Facts       | facts collection, selector, agent detail |
| M7        | Primitives/Runbook    | package/service/file.copy, YAML parser   |
| M8        | Metrics/Logs          | metrics snapshot, log tail streaming     |
| M9        | Drift                 | policy, check, drift report              |
| M10       | Web Admin UI          | agents/jobs/output/facts/drift/audit 화면  |
| M11       | npm/Service 설치        | npm wrapper, demo, systemd install       |
| M12       | MVP hardening         | security checks, docs, smoke tests       |

각 마일스톤은 2~3개 기능 묶음으로 끝나야 한다. 마일스톤 종료 시 사용 가능한 제품 조각이 있어야 한다.

## 3. M0. 프로젝트 골격

목표:

- Rust workspace를 만들고 기본 개발 루프를 확정한다.
- `sponzey --help`, `sponzey controller start`, `sponzey agent --help`가 동작할 수 있는 최소 구조를 만든다.
- settings/logging bootstrap 규칙을 처음부터 넣는다.

### M0.1 Rust workspace 생성

작업:

- root `Cargo.toml` workspace 작성
- crate 생성
  - `crates/fleet-domain`
  - `crates/fleet-application`
  - `crates/fleet-protocol`
  - `crates/fleet-store`
  - `crates/fleet-runner`
  - `crates/fleet-controller`
  - `crates/fleet-agent`
  - `crates/fleet-cli`
- 공통 Rust edition, lint, workspace dependencies 정리
- `README.md`가 없다면 최소 개발 명령 문서 작성
- architecture dependency policy 초안 작성
- 금지 dependency 점검 script 후보 작성

TDD/검증:

- 아직 business behavior가 없으므로 compile check 중심
- `cargo check --workspace`
- `cargo test --workspace`
- `fleet-domain`에 DB/HTTP/async runtime dependency가 없는지 확인

완료 기준:

- 모든 crate가 workspace에 포함된다.
- 빈 테스트가 아니라 최소 compile 가능한 lib/bin entrypoint가 있다.
- domain crate가 outer dependency를 갖지 않는다.
- architecture fitness check를 CI 또는 local script로 실행할 수 있는 경로가 있다.

주의:

- 처음부터 과도한 abstraction을 만들지 않는다.
- workspace만 만들고 기능 구현을 섞지 않는다.

### M0.2 CLI entrypoint와 command skeleton

작업:

- `fleet-cli`에 `sponzey` binary 구성
- `clap` 기반 command tree 작성
  - `sponzey controller init`
  - `sponzey controller start`
  - `sponzey agent enroll`
  - `sponzey agent start`
  - `sponzey agents list`
  - `sponzey enroll-token create`
  - `sponzey run`
  - `sponzey facts`
  - `sponzey metrics`
  - `sponzey logs`
  - `sponzey drift check`
- 아직 구현되지 않은 명령은 명확한 `NotImplemented` 오류 반환

TDD/검증:

- CLI parser unit test
- invalid command test
- help output smoke test

완료 기준:

- `sponzey --help`가 MVP 명령을 보여준다.
- 구현 전 명령은 panic하지 않고 exit code와 메시지를 반환한다.
- CLI parsing은 env var에 의존하지 않는다.

### M0.3 Settings와 logging bootstrap

작업:

- `Settings` typed struct 정의
- `LogProfile` enum 정의
  - `Product`
  - `FieldDebug`
  - `Development`
- CLI args와 최초 config file에서만 settings를 만든다.
- runtime 중 settings 변경 API를 만들지 않는다.
- `tracing` 기반 logging bootstrap 작성
- secret redaction helper skeleton 작성
- `TransportSecurityMode` 정의
  - `TlsRequired`
  - `DevInsecureLoopbackOnly`
- `DevInsecureLoopbackOnly`는 loopback address에서만 유효하도록 validation 작성

TDD/검증:

- settings validation test
- invalid bind addr test
- invalid log profile test
- insecure remote URL rejected test
- insecure loopback accepted test
- redaction helper test
- `std::env::set_var`를 production code에서 사용하지 않는지 검색 확인

완료 기준:

- controller/agent/cli는 bootstrap에서 만든 `Settings`를 명시적으로 전달받는다.
- request handler나 application service에서 env var를 읽지 않는다.
- Product 로그가 기본값이다.
- non-loopback agent/controller URL은 TLS 없이는 validation에서 거부된다.

## 4. M1. Domain/Application 기반

목표:

- agent, job, audit, selector, runbook의 핵심 domain model을 만든다.
- state machine과 validation을 DB/HTTP 없이 테스트한다.

### M1.1 Agent domain model

작업:

- `AgentId`, `AgentName`, `AgentFingerprint`, `AgentLabel` newtype 정의
- `AgentPublicKey`, `AgentIdentity` value object 정의
- `ControllerPublicKey` pinning model 정의
- `Agent` entity 정의
- `AgentStatus` enum 정의
  - `Pending`
  - `Online`
  - `Busy`
  - `Degraded`
  - `Offline`
  - `Disabled`
- `last_seen_at`, `version`, `os`, `arch`, `capabilities` model 정의
- label validation 규칙 작성

TDD:

- valid label test
- invalid label test
- valid agent identity test
- invalid fingerprint test
- pending -> online transition test
- online -> offline transition test
- disabled agent cannot become online without explicit enable test

완료 기준:

- agent state transition이 domain method로만 일어난다.
- stringly typed id를 외부 layer까지 흘리지 않는다.
- agent identity/fingerprint가 enrollment 이후 변경 불가능한 값으로 모델링된다.

### M1.2 Job domain model

작업:

- `JobId`, `TaskId`, `JobTarget`, `JobStatus` 정의
- `TaskEnvelope`, `TaskNonce`, `TaskSignature`, `TaskExpiry` 정의
- `TaskRisk` 정의
  - `Low`
  - `Medium`
  - `High`
- `ApprovalRequirement` 정의
- job state machine 작성
  - `Draft`
  - `PendingApproval`
  - `Queued`
  - `Running`
  - `PartialSuccess`
  - `Success`
  - `Failed`
  - `Canceled`
  - `Expired`
- `JobResultSummary` 정의
- timeout/cancel semantics 정의
- high-risk task는 approval 또는 explicit admin confirmation 없이는 `Queued`로 갈 수 없도록 rule 정의

TDD:

- queued -> running transition
- running -> success transition
- running -> failed transition
- running -> canceled transition
- success 이후 상태 변경 거부
- expired job dispatch 거부
- high-risk task without approval rejected
- signed envelope expiry validation
- envelope target agent mismatch rejected

완료 기준:

- job 상태 변경은 domain rule로 검증된다.
- controller handler가 status string을 직접 바꾸지 않는다.
- task assignment는 envelope 개념으로만 agent에 전달된다.

### M1.3 AuditEvent와 selector model

작업:

- `AuditEvent` 정의
- audit event category 정의
  - agent
  - enrollment
  - job
  - approval
  - drift
  - security
- label selector model 정의
- selector parser 작성
  - `role=web`
  - `role=web,env=prod`
  - `agent:web-01`
- selector matching 작성

TDD:

- selector parse success
- selector parse failure
- label match
- label mismatch
- agent name selector
- audit event redaction test

완료 기준:

- audit event에는 secret 원문이 들어가지 않는다.
- selector는 application layer에서 재사용 가능하다.

## 5. M2. Storage 기반

목표:

- SQLite 저장소와 repository contract를 만든다.
- domain/application은 DB 구현에 직접 의존하지 않는다.

### M2.1 SQLite schema와 migrations

작업:

- SQLite migration 구조 선택
  - SQLx migration 권장
- MVP table 작성
  - `agents`
  - `agent_identities`
  - `controller_identity`
  - `enrollment_tokens`
  - `jobs`
  - `job_targets`
  - `task_assignments`
  - `job_output_chunks`
  - `approval_decisions`
  - `audit_events`
  - `facts_snapshots`
  - `metrics_snapshots`
  - `drift_reports`
- created/updated timestamp 기준 통일
- id는 UUID 또는 ULID 중 하나로 통일
- secret/token raw value를 저장하지 않는 schema로 설계
- public key/fingerprint와 private key 저장 위치를 분리

TDD/검증:

- migration up test
- empty database boot test
- schema compatibility test
- raw token not persisted schema test
- task assignment nonce unique constraint test

완료 기준:

- `sponzey controller init`이 SQLite DB를 만든다.
- migration은 repeatable하게 실행된다.
- controller identity와 agent identity 저장 경계가 명확하다.

### M2.2 Repository traits와 SQLite 구현

작업:

- application layer에 repository trait 정의
  - `AgentRepository`
  - `AgentIdentityRepository`
  - `ControllerIdentityRepository`
  - `EnrollmentTokenRepository`
  - `JobRepository`
  - `TaskAssignmentRepository`
  - `ApprovalRepository`
  - `AuditRepository`
  - `FactsRepository`
  - `MetricsRepository`
  - `DriftRepository`
- SQLite 구현 작성
- repository error type 분리

TDD:

- fake repository로 application test
- SQLite repository contract test
- not found behavior test
- duplicate id behavior test

완료 기준:

- application service는 SQLite concrete type을 모른다.
- SQLite 구현은 infrastructure crate에 갇힌다.

### M2.3 Audit append-only 저장

작업:

- audit event insert only 구현
- audit event update/delete API 만들지 않기
- audit query pagination
- audit category filter

TDD:

- audit insert test
- audit cannot be updated through repository API
- audit pagination test
- secret redaction before storage test

완료 기준:

- MVP 주요 이벤트는 audit writer를 통해 기록 가능하다.
- audit log는 일반 application log와 분리된다.

## 6. M3. Controller API

목표:

- controller가 SQLite DB와 함께 시작된다.
- CLI가 controller REST API를 호출해 enrollment token, agents, jobs를 다룰 준비가 된다.

### M3.1 Controller bootstrap

작업:

- `fleet-controller` library bootstrap 작성. 실행은 단일 `sponzey controller start` subcommand가 담당한다.
- Axum 또는 Actix Web 선택
- bind address, data dir, database URL settings 연결
- `controller init` 시 controller signing key pair 생성
- controller private key 저장 권한 검증
- controller public key fingerprint 출력
- health endpoint
- Product 로그로 startup event 기록
- graceful shutdown

TDD/검증:

- settings validation test
- controller identity created once test
- controller private key permission test
- health endpoint integration test
- startup with temp SQLite test
- shutdown smoke test

완료 기준:

- `sponzey controller start --db sqlite://...`로 서버가 뜬다.
- controller가 시작 중 env var를 직접 읽지 않는다.
- controller signing key 없이 task dispatch를 시작할 수 없다.

### M3.2 Admin bootstrap token

작업:

- local bootstrap admin token 생성 방식 정의
- token은 생성 시 1회만 출력
- token hash 저장
- CLI request authentication header 정의
- MVP는 full RBAC 없이 admin token만 사용

TDD:

- token hash verification
- invalid token rejected
- missing token rejected
- token redaction in logs

완료 기준:

- protected API는 admin token 없이 접근할 수 없다.
- Product 로그에 token 원문이 남지 않는다.

### M3.3 Enrollment token API

작업:

- `POST /api/enrollment-tokens`
- `GET /api/enrollment-tokens`
- `DELETE /api/enrollment-tokens/{id}`
- token TTL, max uses, default labels
- token은 생성 시 원문 1회만 반환
- enrollment response에는 controller public key/fingerprint가 포함된다.
- enrollment token은 agent identity 등록에만 사용하고 task channel 인증에는 재사용하지 않는다.

TDD:

- create token test
- expired token rejected
- revoked token rejected
- max uses exceeded test
- token raw value not stored test
- enrollment response includes controller fingerprint test
- enrollment token cannot authenticate websocket task channel test

완료 기준:

- `sponzey enroll-token create`가 동작한다.
- token 생성/폐기는 audit에 남는다.
- token은 enrollment 이후 agent config에 남지 않는다.

## 7. M4. Agent 연결

목표:

- agent가 controller로 outbound WebSocket 연결을 맺는다.
- enrollment 후 heartbeat와 last seen이 저장된다.

### M4.1 Protocol message schema

작업:

- `fleet-protocol`에 MVP message 정의
  - `EnrollRequest`
  - `EnrollResponse`
  - `AgentHello`
  - `AuthChallenge`
  - `AuthResponse`
  - `AuthAccepted`
  - `Heartbeat`
  - `TaskAssignment`
  - `SignedTaskEnvelope`
  - `OutputChunk`
  - `TaskResult`
  - `FactsSnapshot`
  - `MetricsSnapshot`
  - `LogChunk`
  - `DriftReport`
- protocol version과 message id 포함
- correlation id 포함
- agent id와 target agent id 포함
- auth/session message와 task message 분리
- JSON serialization
- unknown message handling 정책 작성

TDD:

- serialize/deserialize compatibility test
- malformed payload rejected
- unknown message behavior test
- protocol version mismatch test
- auth challenge roundtrip test
- signed task envelope serialization test
- target agent mismatch fixture test

완료 기준:

- protocol fixture가 `docs/protocol.md`와 일치한다.
- authentication message와 task execution message가 섞이지 않는다.

### M4.2 Agent enrollment

작업:

- `sponzey agent enroll --url --token --name --labels`
- agent local identity 생성
- fingerprint 생성
- controller enrollment endpoint
- controller public key/fingerprint pinning
- agent config file 저장
- local config permission check
- raw enrollment token 폐기

TDD:

- enroll success
- invalid token failure
- duplicate agent name policy
- local config permission validation
- fingerprint persistence
- controller fingerprint persisted
- raw enrollment token removed after success
- changed controller fingerprint rejected unless explicit re-enroll

완료 기준:

- enrollment 이후 agent config에 raw token이 남지 않는다.
- controller에 agent가 `Pending` 또는 `Online`으로 등록된다.
- agent는 pinned controller identity와 다른 controller에는 연결하지 않는다.

### M4.3 WebSocket heartbeat

작업:

- agent outbound WebSocket client
- controller WebSocket gateway
- agent hello
- controller auth challenge
- agent challenge signature
- controller verification of agent public key
- heartbeat interval
- last seen update
- reconnect backoff
- online/offline transition
- non-loopback insecure connection 거부

TDD/검증:

- heartbeat message test
- auth challenge success test
- invalid agent signature rejected
- unknown agent id rejected
- pinned controller mismatch rejected
- reconnect policy unit test
- last seen update integration test
- offline transition background job test
- non-loopback insecure websocket rejected test

완료 기준:

- `sponzey agents list`에서 online/offline 상태를 확인할 수 있다.
- agent가 controller로 inbound 접속을 요구하지 않는다.
- authenticated agent만 heartbeat/task channel을 사용할 수 있다.

## 8. M5. Remote Run과 Live Output

목표:

- CLI에서 command를 실행하고 stdout/stderr를 실시간으로 본다.
- job history와 audit가 저장된다.

### M5.1 Command task domain/application

작업:

- `TaskKind::Command` 정의
- command risk level은 기본 `High`
- timeout 필수
- command payload validation
- `--confirm-risk` 또는 Web Admin UI confirmation 없이 high-risk command 생성 거부
- task dispatch use case
- controller-side signed task envelope 생성
- job target 생성

TDD:

- command without timeout rejected
- empty command rejected
- high risk task approval hook path
- high risk command without confirmation rejected
- confirmed high risk command creates audit event
- signed task envelope created with expiry and nonce
- job target created per selected agent

완료 기준:

- command task가 domain model로 표현된다.
- shell 실행 문자열이 handler에서 바로 runner로 가지 않는다.
- task assignment는 controller 서명 없이는 생성되지 않는다.

### M5.2 Agent process runner

작업:

- `fleet-runner`에 command runner 구현
- signed task envelope verification
- nonce replay guard
- target agent id validation
- task expiry validation
- stdout/stderr chunk streaming
- timeout 처리
- cancel 처리
- max output size
- child process env는 per-command explicit env만 허용

TDD/검증:

- successful command test
- non-zero exit code test
- unsigned task rejected
- invalid signature rejected
- expired task rejected
- replayed nonce rejected
- target mismatch rejected
- timeout test
- output chunk order test
- cancel test
- per-command env test

완료 기준:

- runner는 global process env를 변경하지 않는다.
- output은 application log가 아니라 job output storage로 간다.
- agent는 검증 실패 task를 실행하지 않고 security audit event를 보낸다.

### M5.3 CLI run/live output

작업:

- `sponzey run --selector role=web "uptime"`
- high-risk command는 `--confirm-risk` 요구
- REST API job create
- WebSocket 또는 polling 기반 output subscribe
- CLI live output renderer
- job status exit code mapping

TDD/검증:

- CLI run argument parsing test
- CLI high-risk confirmation required test
- selector required/default context behavior test
- e2e local controller+agent command smoke

완료 기준:

- CLI에서 서버별 stdout/stderr가 실시간으로 보인다.
- job success/failure가 저장된다.
- job create/start/complete가 audit에 남는다.
- high-risk execution에는 누가 확인했는지 audit에 남는다.

## 9. M6. Inventory와 Facts

목표:

- agent facts를 수집하고 label selector로 대상 선택을 한다.
- agent detail API와 UI가 facts를 보여준다.

### M6.1 Facts collector

작업:

- OS facts
- CPU facts
- memory facts
- disk facts
- network facts
- hostname/user/runtime facts
- facts snapshot message

TDD/검증:

- parser tests for Linux fixture
- facts serialization test
- facts redaction test

완료 기준:

- `sponzey facts web-01`가 구조화된 facts를 출력한다.
- facts 수집 실패는 agent degraded signal로 이어진다.

### M6.2 Inventory selector application

작업:

- label selector를 job dispatch에 연결
- `agent:name` selector 연결
- no target behavior 정의
- disabled/offline agent target policy 정의

TDD:

- selector returns matching online agents
- disabled agent excluded
- offline agent behavior test
- no target returns explicit error

완료 기준:

- `--selector role=web,env=prod`가 실제 job target을 만든다.
- selector 결과는 audit/debug에 count만 기록한다.

### M6.3 Agent detail API

작업:

- `GET /api/agents`
- `GET /api/agents/{id}`
- `PATCH /api/agents/{id}/labels`
- facts latest endpoint
- label 변경 audit

TDD/검증:

- list agents API test
- get detail API test
- label update validation test
- label update audit test

완료 기준:

- CLI와 UI에서 agent/facts를 확인할 수 있다.

## 10. M7. Task Primitives와 Runbook

목표:

- command 외에 실제 운영에 필요한 최소 primitive를 실행한다.
- YAML runbook을 parsing/validation하고 적용한다.

### M7.1 Runbook parser

작업:

- YAML schema 정의
- `apiVersion`, `kind`, `metadata`, `spec.targets`, `spec.tasks`
- JSON schema export
- validation error formatting
- unsupported field rejection

TDD:

- valid nginx runbook parse
- missing targets rejected
- unsupported task rejected
- invalid YAML error
- schema fixture test

완료 기준:

- `sponzey apply playbook.yml`가 runbook을 읽고 validation한다.
- Ansible full compatibility를 암시하지 않는다.

### M7.2 package/service primitive

작업:

- Linux package manager detection
  - apt
  - dnf/yum
  - apk는 후순위 가능
- package present check
- service status/start/restart/enable for systemd
- changed true/false 반환
- dry-run 준비 구조

TDD/검증:

- command builder unit test
- package already installed fixture
- service status parser test
- systemd unavailable behavior test
- dangerous restart approval hook test

완료 기준:

- nginx install/start runbook이 Linux에서 동작한다.
- primitive output은 구조화된다.

### M7.3 file.copy primitive

작업:

- content upload strategy
- destination validation
- mode/owner/group MVP 범위 정의
- checksum before/after
- atomic write where possible
- path safety guard

TDD:

- copy creates file
- unchanged file returns changed=false
- checksum mismatch handled
- unsafe path rejected
- permission error mapped

완료 기준:

- runbook에서 file copy가 동작한다.
- file write는 audit 가능한 job step으로 남는다.

## 11. M8. Metrics와 Logs

목표:

- 운영 자동화 판단에 필요한 최소 metrics와 log tail을 제공한다.
- Prometheus/Grafana 대체가 아니라 snapshot/stream만 제공한다.

### M8.1 Metrics snapshot

작업:

- CPU usage
- memory usage
- disk usage
- process count
- service status summary
- systemd failed units count
- metrics snapshot storage

TDD/검증:

- Linux `/proc` fixture parser tests
- metrics serialization test
- disk usage threshold formatting test

완료 기준:

- `sponzey metrics web-01`가 최신 snapshot을 출력한다.
- UI에서 최근 snapshot을 볼 수 있다.

### M8.2 Log tail streaming

작업:

- file log tail
- journald adapter skeleton
- max line size
- max stream duration
- cancel stream
- redaction 적용

TDD/검증:

- tail existing file test
- follow appended line test
- cancel tail test
- redaction test
- file not found error test

완료 기준:

- `sponzey logs web-01 --file /var/log/syslog`가 streaming된다.
- `sponzey logs nginx` shortcut은 systemd/journald 가능 시 동작한다.

### M8.3 Retention policy

작업:

- job output retention 기본값
- log stream artifact retention 기본값
- metrics snapshot retention 기본값
- retention cleanup command

TDD:

- retention cutoff test
- cleanup dry-run test
- audit cleanup event test

완료 기준:

- MVP가 무제한 로그 저장으로 디스크를 채우지 않는다.

## 12. M9. Drift Detection

목표:

- 선언한 상태와 실제 상태 차이를 감지한다.
- nginx running policy를 MVP demo로 제공한다.

### M9.1 Policy model

작업:

- `Policy` domain model
- selector
- checks
  - service state
  - package present
  - file checksum
- remediation block은 MVP에서 manual proposal까지만

TDD:

- valid policy parse
- invalid selector rejected
- unsupported check rejected
- remediation without approval rejected

완료 기준:

- policy YAML을 validation할 수 있다.

### M9.2 Agent-side check

작업:

- service running check
- package present check
- file checksum check
- expected/actual report
- drift status
  - compliant
  - drifted
  - unknown

TDD/검증:

- service running fixture
- service stopped fixture
- file checksum mismatch
- unknown check behavior

완료 기준:

- nginx stopped 상태를 drift로 감지한다.

### M9.3 Drift CLI/API/UI 연결

작업:

- `sponzey drift check --policy nginx-running`
- drift check job dispatch
- drift report storage
- drift result audit
- UI drift diff rendering

TDD/검증:

- CLI parse test
- drift report API test
- e2e drift smoke test

완료 기준:

- expected/actual diff가 CLI와 UI에 보인다.
- drift 감지는 remediation 실행과 분리된다.

## 13. M10. Web Admin UI

목표:

- 얇은 Web Admin UI로 MVP 기능을 확인하고 실행한다.
- UI는 domain rule을 재구현하지 않는다.

### M10.1 Web Admin UI scaffold

작업:

- React/Vite 또는 Svelte static export 선택
- `/admin` static serving
- generated API client 또는 shared schema 기반 client
- basic layout
- auth token 입력/저장 정책

TDD/검증:

- UI build
- API client type check
- static asset serving integration test

완료 기준:

- controller가 `/admin`에서 UI를 제공한다.
- 별도 Node.js web server가 필요 없다.

### M10.2 Agents/Jobs screens

작업:

- agents table
- agent detail
- run command form
- job history
- job detail
- live output viewer

TDD/검증:

- agents table render test
- run command form validation test
- live output component test
- dangerous command confirmation test

완료 기준:

- 브라우저에서 agent를 선택해 command를 실행하고 output을 본다.

### M10.3 Facts/Metrics/Drift/Audit screens

작업:

- facts panel
- metrics snapshot panel
- drift report diff
- audit event list
- product/field debug log와 audit의 차이 UI 설명

TDD/검증:

- facts render test
- metrics render test
- drift diff render test
- audit list render test

완료 기준:

- MVP demo를 UI만으로 설명할 수 있다.

## 14. M11. npm 설치와 서비스 설치

목표:

- Rust binary를 npm으로 설치하는 UX를 만든다.
- Linux systemd service install을 제공한다.

### M11.1 npm binary wrapper

작업:

- `npm/fleet/package.json`
- platform optional dependency package 구조
- bin shim
- local development pack script
- version sync with Cargo

TDD/검증:

- npm package script test
- bin shim points to Rust binary
- unsupported platform error test

완료 기준:

- `npm install -g @sponzey/fleet` 후 `sponzey --help`가 실행된다.
- npm은 runtime application이 아니라 binary distribution wrapper다.

### M11.2 npx demo

작업:

- `npx @sponzey/fleet demo`
- temp data dir
- local controller
- local agent
- `DevInsecureLoopbackOnly` mode 명시
- loopback URL만 허용
- sample command
- browser URL output

TDD/검증:

- demo command smoke
- temp file cleanup behavior
- port conflict behavior
- non-loopback demo insecure rejected test

완료 기준:

- 5분 안에 local demo가 된다.
- demo mode의 insecure transport는 Product 로그와 audit에 명확히 남는다.

### M11.3 systemd install

작업:

- `sponzey agent install-service`
- `sponzey agent start-service`
- `sponzey controller install-service`
- service file template
- absolute binary path pinning
- user/group option

TDD/검증:

- service file render test
- invalid user rejected
- dry-run output test

완료 기준:

- Linux에서 재부팅 후 agent/controller가 자동 시작 가능하다.
- sudo/root가 필요한 작업은 명확히 실패/안내한다.

## 15. M12. MVP Hardening

목표:

- MVP를 공개 가능한 수준으로 다듬는다.
- 보안, 로그, 설정, 문서, smoke test를 마무리한다.

### M12.1 Security review

작업:

- token redaction review
- command output logging review
- audit coverage review
- local config permission review
- dangerous task classification review
- timeout/cancel review
- controller signing key review
- agent identity proof review
- signed task envelope verification review
- insecure loopback-only mode review

검증:

- product log에 secret이 없는지 fixture test
- audit coverage checklist
- command output이 app log에 섞이지 않는지 test
- unsigned/invalid/expired/replayed task rejection tests
- non-loopback insecure URL rejection test

완료 기준:

- MVP 보안 체크리스트가 통과한다.

### M12.2 Configuration review

작업:

- `std::env::var` 사용 위치 audit
- `std::env::set_var` production code 금지 확인
- settings bootstrap path 문서화
- CLI arg/config file precedence 문서화
- runtime config mutation endpoint 없음 확인

검증:

- code search
- settings tests
- docs review

완료 기준:

- 외부 환경 상수는 bootstrap에서만 읽힌다.
- 설정 변경은 restart 또는 명시적 command로만 가능하다.

### M12.3 End-to-end smoke suite

작업:

- local controller start
- local agent enroll
- heartbeat 확인
- command run
- facts
- metrics
- logs
- drift check
- audit query
- Web Admin UI static serving

검증:

- `scripts/smoke_mvp.sh` 또는 equivalent 작성
- CI에서 root 권한 없이 가능한 부분만 기본 실행
- root/systemd 부분은 별도 manual smoke로 분리

완료 기준:

- fresh checkout에서 MVP smoke가 재현된다.

## 16. 작업 우선순위

반드시 이 순서를 따른다.

1. M0 프로젝트 골격
2. M1 domain/application model
3. M2 storage
4. M3 controller API
5. M4 agent connection
6. M5 remote run
7. M6 inventory/facts
8. M7 runbook/primitives
9. M8 metrics/logs
10. M9 drift
11. M10 Web Admin UI
12. M11 npm/service install
13. M12 hardening

순서를 바꿀 수 있는 경우:

- Web Admin UI scaffold는 M5 이후 병렬 가능
- npm wrapper는 M0 이후 skeleton만 먼저 가능
- systemd install은 M4 이후 병렬 가능

순서를 바꾸면 안 되는 경우:

- storage 없이 controller API를 임시 in-memory로 크게 구현하지 않는다.
- domain state machine 없이 job dispatch를 구현하지 않는다.
- enrollment token 테스트 없이 agent 연결을 구현하지 않는다.
- command runner security boundary 없이 live output을 구현하지 않는다.
- drift model 없이 remediation을 구현하지 않는다.

## 17. MVP Definition of Done

MVP 완료 조건:

- `cargo fmt` 통과
- `cargo clippy --workspace --all-targets` 통과
- `cargo test --workspace` 통과
- Web Admin UI build 통과
- local smoke test 통과
- docs updated
- Product 로그 기본값 확인
- FieldDebug 로그에서 secret redaction 확인
- Development 로그가 production 기본값이 아님을 확인
- env var 중간 변경 코드 없음
- runtime config mutation endpoint 없음
- non-loopback insecure transport 거부
- controller public key pinning 동작
- authenticated agent만 WebSocket task channel 사용 가능
- unsigned/invalid/expired/replayed task 거부
- high-risk command는 explicit confirmation 없이는 실행 불가
- audit 이벤트 누락 없음
- command output과 application log 분리
- SQLite migration repeatable
- npm wrapper로 `sponzey --help` 가능
- controller/agent/service install 문서화

MVP demo script:

```bash
npm install -g @sponzey/fleet
sponzey controller init --db sqlite://./fleet.db
sponzey controller start --host 127.0.0.1 --port 7700 --dev-insecure-loopback
sponzey enroll-token create --labels role=web,env=dev
sponzey agent enroll --url http://127.0.0.1:7700 --token <token> --name web-01 --labels role=web,env=dev
sponzey agent start --dev-insecure-loopback
sponzey agents list
sponzey run --selector role=web --confirm-risk "uptime"
sponzey facts web-01
sponzey metrics web-01
sponzey drift check --policy examples/policies/nginx-running.yml
```

## 18. 리스크와 대응

### 18.1 Rust 개발 속도

리스크:

- 초기 구현 속도가 TypeScript 단일 스택보다 느릴 수 있다.

대응:

- domain/application 테스트를 먼저 만들고 외부 layer를 얇게 유지한다.
- UI는 TypeScript static UI로 빠르게 만든다.
- MVP primitive 범위를 엄격히 제한한다.

### 18.2 Agent 보안

리스크:

- root 권한 command runner는 사고 영향 범위가 크다.

대응:

- timeout, cancel, output limit, dangerous classification을 M5에서 바로 구현한다.
- Product 로그에는 output 원문을 남기지 않는다.
- command execution은 primitive boundary를 거친다.

### 18.3 설정 복잡도

리스크:

- controller/agent/CLI 설정이 파일과 env에 흩어질 수 있다.

대응:

- bootstrap 시점 settings object로 고정한다.
- runtime config mutation을 만들지 않는다.
- config file은 최소화하고 key를 문서화한다.

### 18.4 Web Admin UI 비대화

리스크:

- UI가 dashboard builder나 workflow designer로 커질 수 있다.

대응:

- agents, jobs, live output, facts/metrics, drift, audit만 MVP 범위로 둔다.
- domain rule은 UI에 두지 않는다.
- controller static serving만 사용한다.

### 18.5 Observability 범위 팽창

리스크:

- metrics/logs 기능이 Prometheus/Grafana 대체로 커질 수 있다.

대응:

- snapshot과 tail만 제공한다.
- retention을 기본 적용한다.
- export는 후속 단계로 미룬다.

## 19. 산출물 목록

MVP 완료 시 있어야 할 산출물:

- Rust workspace
- 단일 `sponzey` binary
- Controller 역할 subcommand
- Agent 역할 subcommand
- CLI 운영 subcommand
- npm binary wrapper
- SQLite migrations
- protocol docs
- API docs
- runbook schema docs
- sample runbooks
- sample policies
- Web Admin UI static build
- local demo command
- smoke test script
- systemd service template
- security checklist
- configuration guide
- logging guide
- MVP release notes

## 20. 첫 구현 체크리스트

바로 시작할 때의 구체적 순서:

1. `Cargo.toml` workspace 생성
2. `fleet-domain` crate 생성
3. `fleet-cli` crate 생성
4. `sponzey --help` skeleton 작성
5. `Settings`와 `LogProfile` test 작성
6. settings/logging bootstrap 구현
7. `TransportSecurityMode` validation test 작성
8. loopback-only insecure mode 구현
9. `AgentIdentity` domain test 작성
10. `Agent` domain model 구현
11. `TaskEnvelope`와 `Job` state machine test 작성
12. `Job` domain model 구현
13. `AuditEvent`와 selector test 작성
14. audit/selector 구현
15. controller/agent identity storage migration skeleton 작성
16. SQLite migration skeleton 작성
17. repository trait 작성
18. fake repository로 application test 작성

첫 번째 실제 feature는 agent enrollment가 아니라 CLI/settings/domain skeleton이다.  
이 순서를 지키면 이후 WebSocket, command runner, drift detection이 무리 없이 올라간다.
