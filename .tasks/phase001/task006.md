# Task 006. Remote Run과 Live Output

상위 계획: `M5. Remote Run과 Live Output`  
목표: signed task envelope 기반 command 실행과 live stdout/stderr streaming을 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] command high-risk `--confirm-risk` guard 구현
- [x] `fleet-runner` command runner skeleton 구현
- [x] TaskEnvelope validation helper 구현
- [x] CLI local command execution smoke 통과
- [x] authenticated WebSocket heartbeat channel 구현
- [x] controller key pair와 agent key pair 생성/저장/검증 기반 마련
- [x] `TaskKind::Command`와 `CommandTask` domain model 구현
- [x] command job 생성 application use case와 signed envelope 생성 경계 구현
- [x] command job 생성 audit event 구현
- [x] runner command timeout/output limit/per-command env 구현
- [x] runner signed envelope verification과 nonce replay guard 구현
- [x] completed output chunking skeleton 구현
- [x] runner cancel hook과 cancel test 구현
- [x] CLI command output redaction test 구현
- [x] SQLite task assignment 저장 구현
- [x] SQLite job output chunk 저장/조회 구현
- [x] `/api/jobs/command` REST API 구현
- [x] command job REST API high-risk confirmation test 구현
- [x] controller-agent remote command dispatch 구현
- [x] agent-side signed command assignment 실행 구현
- [x] command output chunk/result WebSocket 수신 및 저장 구현
- [x] local controller+agent remote command smoke 추가
- [x] agent task 검증 실패 security event 보고 구현
- [x] polling 방식 output 조회 API 구현
- [x] Web Admin UI output viewer와 recent job history 연결
- [x] streaming protocol 기반 live output 구현
- [x] agent command runner가 stdout/stderr chunk를 process 종료 전 callback으로 전송하도록 전환

## 1. 목적

MVP의 첫 번째 실제 운영 가치는 원격 명령 실행이다. 그러나 command 실행은 high-risk action이므로 signed envelope, explicit confirmation, timeout, cancel, output limit을 처음부터 포함한다.

기능 묶음:

- Command task domain/application
- Agent process runner
- CLI run/live output

## 2. 선행 조건

- [x] Task 005 완료
- [x] authenticated WebSocket channel이 동작한다.
- [x] controller signing key와 agent identity가 준비되어 있다.

## 3. 기능 묶음 A. Command task domain/application

### 작업

- [x] `TaskKind::Command` 정의
- [x] command risk level을 기본 `High`로 둔다.
- [x] timeout을 필수로 둔다.
- [x] command payload validation 구현
- [x] `--confirm-risk` 또는 Web Admin UI confirmation 없이 high-risk command 생성을 거부한다.
- [x] task dispatch use case 구현
- [x] controller-side signed task envelope 생성
- [x] job target 생성
- [x] task expiry와 nonce 생성
- [x] job create/start/complete audit event 생성

### TDD

- [x] command without timeout rejected
- [x] empty command rejected
- [x] high risk task approval hook path
- [x] high risk command without confirmation rejected
- [x] confirmed high risk command creates audit event
- [x] signed task envelope created with expiry and nonce
- [x] job target created per selected agent

### 완료 기준

- [x] command task가 domain model로 표현된다.
- [x] shell 실행 문자열이 handler에서 바로 runner로 가지 않는다.
- [x] task assignment는 controller 서명 없이는 생성되지 않는다.

## 4. 기능 묶음 B. Agent process runner

### 작업

- [x] `fleet-runner`에 command runner 구현
- [x] signed task envelope verification 구현
- [x] nonce replay guard 구현
- [x] target agent id validation 구현
- [x] task expiry validation 구현
- [x] stdout/stderr completed chunk 전송 구현
- [x] timeout 처리 구현
- [x] cancel 처리 구현
- [x] max output size 구현
- [x] child process env는 per-command explicit env만 허용한다.
- [x] global process env를 변경하지 않는다.

### TDD/검증

- [x] successful command test
- [x] non-zero exit code test
- [x] unsigned task rejected
- [x] invalid signature rejected
- [x] expired task rejected
- [x] replayed nonce rejected
- [x] target mismatch rejected
- [x] timeout test
- [x] output chunk order test
- [x] cancel test
- [x] per-command env test

### 완료 기준

- [x] runner는 global process env를 변경하지 않는다.
- [x] output은 application log가 아니라 job output storage로 간다.
- [x] agent는 검증 실패 task를 실행하지 않는다.
- [x] agent는 검증 실패 task에 대해 controller security audit event를 보낸다.

## 5. 기능 묶음 C. CLI run/live output

### 작업

- [x] `sponzey run --selector role=web --confirm-risk "uptime"` 구현
- [x] high-risk command는 `--confirm-risk`를 요구한다.
- [x] REST API job create 구현
- [x] output subscribe 구현

  - [x] WebSocket 방식 또는 polling 방식 중 MVP에 적합한 방식 선택
- [x] CLI live output renderer 구현
- [x] 서버별 stdout/stderr prefix format 정의
- [x] job status exit code mapping 구현

### TDD/검증

- [x] CLI run argument parsing test
- [x] CLI high-risk confirmation required test
- [x] selector required/default context behavior test
- [x] e2e local controller+agent command smoke
- [x] output redaction test

### 완료 기준

- [x] CLI에서 서버별 stdout/stderr가 polling renderer로 보인다.
- [x] Agent는 실행 중 stdout/stderr chunk를 controller에 즉시 전송한다.
- [x] job success/failure가 저장된다.
- [x] job create/start/complete가 audit에 남는다.
- [x] high-risk execution에는 누가 확인했는지 audit에 남는다.

## 6. 완료 전 체크

- [x] command output은 Product 로그에 남지 않는다.
- [x] signed envelope 검증 실패는 실행 전 차단된다.
- [x] timeout/cancel/output limit이 모두 동작한다.
- [x] high-risk confirmation 없이 command를 실행할 수 없다.
