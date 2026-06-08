# Task 002. Domain/Application 기반 모델

상위 계획: `M1. Domain/Application 기반`  
목표: Agent, Job, AuditEvent, Selector의 core domain model과 state machine을 만든다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 1. 목적

Controller API, WebSocket, SQLite 구현 전에 순수 domain/application 모델을 먼저 만든다. 이 task는 DB, HTTP, filesystem 없이 테스트 가능한 핵심 규칙을 만든다.

기능 묶음:

- Agent identity와 상태 모델
- Job, TaskEnvelope, high-risk 실행 모델
- AuditEvent와 selector 모델

## 2. 선행 조건

- [x] Task 001 완료
- [x] `fleet-domain`이 outer dependency 없이 존재한다.
- [x] `fleet-application`이 domain crate를 참조할 수 있다.

## 진행 메모

- [x] Agent identity/status/label 모델 구현
- [x] Job state machine, TaskEnvelope, TaskRisk, ApprovalRequirement 구현
- [x] AuditEvent, AuditCategory, secret-safe audit value 구현
- [x] Selector parser/matcher 구현
- [x] `fleet-application` repository trait과 `EnrollAgent` skeleton 구현
- [x] 관련 domain/application 테스트 추가 및 통과

## 3. 기능 묶음 A. Agent domain model

### 작업

- [x] `AgentId` newtype 정의
- [x] `AgentName` newtype 정의
- [x] `AgentFingerprint` newtype 정의
- [x] `AgentLabel` newtype 정의
- [x] `AgentPublicKey` value object 정의
- [x] `AgentIdentity` value object 정의
- [x] `ControllerPublicKey` pinning model 정의
- [x] `Agent` entity 정의
- [x] `AgentStatus` enum 정의

  - [x] `Pending`
  - [x] `Online`
  - [x] `Busy`
  - [x] `Degraded`
  - [x] `Offline`
  - [x] `Disabled`
- [x] `last_seen_at`, `version`, `os`, `arch`, `capabilities` 모델 정의
- [x] label validation 규칙 정의
- [x] agent identity/fingerprint는 enrollment 이후 불변으로 모델링한다.

### TDD

- [x] valid label test
- [x] invalid label test
- [x] valid agent identity test
- [x] invalid fingerprint test
- [x] pending -> online transition test
- [x] online -> offline transition test
- [x] disabled agent cannot become online without explicit enable test

### 완료 기준

- [x] agent state transition은 domain method로만 일어난다.
- [x] string id가 layer 사이를 무방비로 흐르지 않는다.
- [x] agent identity 변경은 명시적 re-enroll flow 없이는 불가능하다.

## 4. 기능 묶음 B. Job, TaskEnvelope, high-risk 실행 모델

### 작업

- [x] `JobId` newtype 정의
- [x] `TaskId` newtype 정의
- [x] `JobTarget` 정의
- [x] `JobStatus` enum 정의

  - [x] `Draft`
  - [x] `PendingApproval`
  - [x] `Queued`
  - [x] `Running`
  - [x] `PartialSuccess`
  - [x] `Success`
  - [x] `Failed`
  - [x] `Canceled`
  - [x] `Expired`
- [x] `TaskEnvelope` 정의
- [x] `TaskNonce` 정의
- [x] `TaskSignature` 정의
- [x] `TaskExpiry` 정의
- [x] `TaskRisk` 정의

  - [x] `Low`
  - [x] `Medium`
  - [x] `High`
- [x] `ApprovalRequirement` 정의
- [x] `JobResultSummary` 정의
- [x] timeout/cancel semantics 정의
- [x] high-risk task는 approval 또는 explicit admin confirmation 없이는 `Queued`로 갈 수 없게 한다.

### TDD

- [x] queued -> running transition
- [x] running -> success transition
- [x] running -> failed transition
- [x] running -> canceled transition
- [x] success 이후 상태 변경 거부
- [x] expired job dispatch 거부
- [x] high-risk task without approval rejected
- [x] signed envelope expiry validation
- [x] envelope target agent mismatch rejected

### 완료 기준

- [x] job 상태 변경은 domain rule로 검증된다.
- [x] controller handler가 status string을 직접 바꾸지 않는다.
- [x] agent로 전달되는 task는 envelope 개념으로만 표현된다.

## 5. 기능 묶음 C. AuditEvent와 selector model

### 작업

- [x] `AuditEvent` 정의
- [x] audit category 정의

  - [x] agent
  - [x] enrollment
  - [x] job
  - [x] approval
  - [x] drift
  - [x] security
- [x] audit actor model 정의
- [x] audit target model 정의
- [x] secret reference와 redacted value 표현을 분리한다.
- [x] label selector model 정의
- [x] selector parser 작성

  - [x] `role=web`
  - [x] `role=web,env=prod`
  - [x] `agent:web-01`
- [x] selector matching 작성

### TDD

- [x] selector parse success
- [x] selector parse failure
- [x] label match
- [x] label mismatch
- [x] agent name selector
- [x] audit event redaction test
- [x] security audit event construction test

### 완료 기준

- [x] audit event에는 secret 원문이 들어가지 않는다.
- [x] selector는 application layer에서 재사용 가능하다.
- [x] security-sensitive event를 audit로 표현할 수 있다.

## 6. Application skeleton

- [x] `fleet-application`에 use case input/output type을 둔다.
- [x] 아직 DB나 HTTP 구현을 넣지 않는다.
- [x] repository는 trait boundary만 예상한다.
- [x] fake repository 기반 테스트 구조를 준비한다.

## 7. 완료 전 체크

- [x] domain crate가 `tokio`, `sqlx`, `axum`, `reqwest`에 의존하지 않는다.
- [x] 모든 핵심 state transition test가 있다.
- [x] high-risk 실행 경계가 domain rule에 들어 있다.
- [x] task envelope 개념이 domain에 존재한다.