# Task 003. SQLite Storage와 Repository

상위 계획: `M2. Storage 기반`  
목표: SQLite migration, repository contract, append-only audit 저장소를 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] `AgentRepository` trait 구현
- [x] `MemoryAgentRepository` contract skeleton 구현
- [x] duplicate/not found 기본 동작 테스트 추가
- [x] SQLite migration과 실제 SQLite repository 구현
- [x] controller/agent identity schema 구현
- [x] append-only audit repository 구현
- [x] SQLite `JobRepository`, `TaskAssignmentRepository`, `JobOutputRepository` 구현
- [x] task assignment와 job output chunk 저장 테스트 추가
- [x] MVP command/inventory schema compatibility test 추가
- [x] missing agent/job/facts 조회가 `None`으로 닫히는 contract test 추가
- [x] identity/token/approval/facts/metrics/drift repository trait와 SQLite contract 구현
- [x] runtime 생성 id를 prefixed ULID 기준으로 통일

## 1. 목적

Domain/Application이 infrastructure에 의존하지 않도록 repository trait과 SQLite 구현을 분리한다. MVP는 SQLite를 기본 저장소로 사용하되, Postgres 확장을 고려해 schema와 repository boundary를 설계한다.

기능 묶음:

- SQLite schema/migration
- Repository traits와 SQLite 구현
- Audit append-only storage

## 2. 선행 조건

- [x] Task 001 완료
- [x] Task 002 완료
- [x] Agent, Job, AuditEvent, Selector domain model이 존재한다.

## 3. 기능 묶음 A. SQLite schema와 migration

### 작업

- [x] SQLx migration 또는 동등한 migration 도구를 선택한다.
- [x] `sponzey controller init`이 migration을 실행할 수 있게 설계한다.
- [x] MVP table을 작성한다.

  - [x] `agents`
  - [x] `agent_identities`
  - [x] `controller_identity`
  - [x] `enrollment_tokens`
  - [x] `jobs`
  - [x] `job_targets`
  - [x] `task_assignments`
  - [x] `job_output_chunks`
  - [x] `approval_decisions`
  - [x] `audit_events`
  - [x] `facts_snapshots`
  - [x] `metrics_snapshots`
  - [x] `drift_reports`
- [x] timestamp 기준을 통일한다.
- [x] id는 UUID 또는 ULID 중 하나로 통일한다. MVP runtime 생성 id는 prefixed ULID를 사용한다.
- [x] enrollment token raw value를 저장하지 않는 schema를 만든다.
- [x] public key/fingerprint와 private key 저장 위치를 분리한다.
- [x] task assignment nonce unique constraint를 둔다.

### 테스트/검증

- [x] migration up test
- [x] empty database boot test
- [x] schema compatibility test
- [x] raw token not persisted schema test
- [x] task assignment nonce unique constraint test

### 완료 기준

- [x] SQLite DB가 repeatable하게 초기화된다.
- [x] controller identity와 agent identity 저장 경계가 명확하다.
- [x] token 원문 저장 경로가 없다.

## 4. 기능 묶음 B. Repository traits와 SQLite 구현

### 작업

- [x] application layer에 repository trait을 정의한다.

  - [x] `AgentRepository`
  - [x] `AgentIdentityRepository`
  - [x] `ControllerIdentityRepository`
  - [x] `EnrollmentTokenRepository`
  - [x] `JobRepository`
  - [x] `TaskAssignmentRepository`
  - [x] `ApprovalRepository`
  - [x] `AuditRepository`
  - [x] `FactsRepository`
  - [x] `MetricsRepository`
  - [x] `DriftRepository`
  - [x] `JobOutputRepository`
- [x] `fleet-store`에 SQLite 구현을 둔다.
- [x] repository error type을 domain/application error와 분리한다.
- [x] duplicate key, not found, constraint violation을 명확히 매핑한다.
- [x] repository는 secret redaction 책임을 떠안지 않는다. 저장 전 application에서 redaction한다.

### TDD

- [x] fake repository 기반 application test
- [x] SQLite repository contract test
- [x] not found behavior test
- [x] duplicate id behavior test
- [x] constraint violation mapping test

### 완료 기준

- [x] application service는 SQLite concrete type을 모른다.
- [x] SQLite 구현은 infrastructure crate에 갇힌다.
- [x] repository trait은 test fake로 대체 가능하다.

## 5. 기능 묶음 C. Audit append-only 저장

### 작업

- [x] audit event insert only 구현
- [x] audit update/delete repository API를 만들지 않는다.
- [x] audit query pagination 구현
- [x] audit category filter 구현
- [x] security event category를 저장할 수 있게 한다.
- [x] audit와 application log를 분리한다.

### TDD

- [x] audit insert test
- [x] audit cannot be updated through repository API
- [x] audit pagination test
- [x] category filter test
- [x] secret redaction before storage test

### 완료 기준

- [x] MVP 주요 이벤트는 audit writer를 통해 기록 가능하다.
- [x] audit는 append-only API만 갖는다.
- [x] token, private key, command secret이 audit에 원문으로 들어가지 않는다.

## 6. 완료 전 체크

- [x] migration은 fresh DB에서 통과한다.
- [x] migration은 이미 초기화된 DB에서 안전하게 동작한다.
- [x] repository는 application trait 뒤에 숨겨져 있다.
- [x] test DB가 workspace 외부 상태에 의존하지 않는다.
