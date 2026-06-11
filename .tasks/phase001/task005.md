# Task 005. Agent 연결과 인증된 WebSocket

상위 계획: `M4. Agent 연결`  
목표: agent enrollment, controller pinning, authenticated outbound WebSocket, heartbeat를 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] protocol message enum skeleton 구현
- [x] auth/session/task message type 분리 skeleton 구현
- [x] JSON wire schema와 serde encode/decode 구현
- [x] unknown message/version mismatch rejection test 추가
- [x] `docs/protocol.md` 작성
- [x] `sponzey agent enroll` 로컬 config skeleton 구현
- [x] controller fingerprint pinning 값 저장 skeleton 구현
- [x] Controller `/api/agents/enroll` endpoint 구현
- [x] enrollment token hash consume/use-count/expiry validation 구현
- [x] CLI `sponzey agent enroll`이 Controller enrollment endpoint 호출
- [x] WebSocket gateway/client MVP path 구현
- [x] auth challenge/signature roundtrip MVP 구현
- [x] heartbeat 1회 전송과 agent online transition 구현
- [x] loopback 외 insecure controller URL 거부
- [x] WebSocket auth failure security audit event 기록
- [x] 장기 daemon heartbeat/reconnect loop 구현
- [x] Ed25519 agent key pair 생성과 challenge signature verification 구현
- [x] local agent config/private key permission validation 구현
- [x] pinned controller fingerprint mismatch 거부 구현
- [x] invalid signature, unknown agent, fingerprint mismatch reject/audit test 추가

## 1. 목적

Agent는 private network 뒤에서 controller로 outbound 연결한다. 이 연결은 단순 WebSocket이 아니라 agent identity proof 이후에만 heartbeat/task channel을 허용해야 한다.

기능 묶음:

- Protocol message schema
- Agent enrollment와 controller pinning
- Authenticated WebSocket heartbeat

## 2. 선행 조건

- [x] Task 004의 controller identity/enrollment API 범위 완료
- [x] controller identity와 enrollment token API가 준비되어 있다.
- [x] controller URL validation과 insecure HTTP warning이 구현되어 있다.

## 3. 기능 묶음 A. Protocol message schema

### 작업

- [x] `fleet-protocol`에 MVP message를 정의한다.

  - [x] `EnrollRequest`
  - [x] `EnrollResponse`
  - [x] `AgentHello`
  - [x] `AuthChallenge`
  - [x] `AuthResponse`
  - [x] `AuthAccepted`
  - [x] `Heartbeat`
  - [x] `TaskAssignment`
  - [x] `SignedTaskEnvelope`
  - [x] `OutputChunk`
  - [x] `TaskResult`
  - [x] `FactsSnapshot`
  - [x] `MetricsSnapshot`
  - [x] `LogChunk`
  - [x] `DriftReport`
- [x] protocol version을 포함한다.
- [x] message id를 포함한다.
- [x] correlation id를 포함한다.
- [x] agent id와 target agent id를 포함한다.
- [x] auth/session message와 task message를 분리한다.
- [x] JSON serialization을 구현한다.
- [x] unknown message handling 정책을 문서화한다.

### TDD

- [x] serialize/deserialize compatibility test
- [x] malformed payload rejected
- [x] unknown message behavior test
- [x] protocol version mismatch test
- [x] auth challenge roundtrip test
- [x] signed task envelope serialization test
- [x] target agent mismatch fixture test

### 완료 기준

- [x] protocol fixture가 `docs/protocol.md`와 일치한다.
- [x] authentication message와 task execution message가 섞이지 않는다.

## 4. 기능 묶음 B. Agent enrollment와 controller pinning

### 작업

- [x] `sponzey agent enroll --url --token --name --labels` 구현
- [x] agent local key pair 생성
- [x] agent fingerprint 생성
- [x] controller enrollment endpoint 호출
- [x] controller public key/fingerprint pinning
- [x] agent config file 저장
- [x] local config permission check
- [x] raw enrollment token 폐기
- [x] changed controller fingerprint는 explicit re-enroll 없이는 거부

### TDD

- [x] enroll success
- [x] invalid token failure
- [x] duplicate agent name policy
- [x] local config permission validation
- [x] fingerprint persistence
- [x] controller fingerprint persisted
- [x] raw enrollment token removed after success
- [x] changed controller fingerprint rejected unless explicit re-enroll

### 완료 기준

- [x] enrollment 이후 agent config에 raw token이 남지 않는다.
- [x] controller에 agent가 `Pending` 또는 `Online`으로 등록된다.
- [x] agent는 pinned controller identity와 다른 controller에는 연결하지 않는다.

## 5. 기능 묶음 C. Authenticated WebSocket heartbeat

### 작업

- [x] agent outbound WebSocket client 구현
- [x] controller WebSocket gateway 구현
- [x] agent hello message 구현
- [x] controller auth challenge 구현
- [x] agent challenge signature 구현
- [x] controller verification of agent public key 구현
- [x] heartbeat interval 구현
- [x] last seen update 구현
- [x] reconnect backoff 구현
- [x] online/offline transition 구현
- [x] non-loopback HTTP connection warning

### TDD/검증

- [x] heartbeat message test
- [x] auth challenge success test
- [x] invalid agent signature rejected
- [x] unknown agent id rejected
- [x] pinned controller mismatch rejected
- [x] reconnect policy unit test
- [x] last seen update integration test
- [x] offline transition background job test
- [x] non-loopback HTTP websocket warning test

### 완료 기준

- [x] `sponzey agents list`에서 online/offline 상태를 확인할 수 있다.
- [x] agent가 controller로 inbound 접속을 요구하지 않는다.
- [x] authenticated agent만 heartbeat/task channel을 사용할 수 있다.

## 6. 완료 전 체크

- [x] enrollment token이 task channel 인증에 재사용되지 않는다.
- [x] WebSocket 연결은 agent identity proof를 요구한다.
- [x] loopback demo 외 insecure URL은 거부된다.
- [x] auth 실패는 security audit event로 남는다.
