# Task 004. Controller API와 Bootstrap

상위 계획: `M3. Controller API`  
목표: Controller bootstrap, admin token, enrollment token API를 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] `sponzey controller init` 로컬 skeleton 구현
- [x] controller dev key 파일 생성 skeleton 구현
- [x] `sponzey controller start` HTTP warning/audit 구현
- [x] `controller init`이 SQLite schema migration 실행
- [x] 최소 REST API 서버와 health endpoint 구현
- [x] admin bootstrap token 생성/저장/검증 구현
- [x] enrollment token create/list/revoke API skeleton 구현
- [x] public controller identity endpoint 구현
- [x] enrollment response controller public key/fingerprint 포함
- [x] controller init 실제 Ed25519 key pair 생성
- [x] controller private key 0600 권한 검증
- [x] enrollment token default labels 적용 및 explicit labels override 구현
- [x] enrollment token create/revoke audit 기록
- [x] enrollment token은 WebSocket 인증에 재사용 불가 테스트 추가
- [x] Axum 기반 HTTP/WebSocket server 전환 완료
- [x] 기존 REST/static route core는 Axum fallback adapter로 보존
- [x] agent WebSocket `/api/agents/ws`는 Axum WebSocket handler로 전환
- [x] enrollment token API TTL/use-count validation 구현
- [x] controller private signing key 없이는 server identity load 실패
- [x] `/api/jobs/command` handler는 application use case adapter를 통해 job 생성
- [x] controller accept loop에 명시적 shutdown signal 경계 구현
- [x] shutdown smoke test 추가
- [x] `sponzey controller start --db sqlite://...` explicit DB argument 구현
- [x] enrollment token create/list/revoke API를 application use case adapter 경유로 전환
- [x] admin token verification과 REST 조회 helper를 application use case adapter 경유로 전환
- [x] HTTP framework 전환 대상은 Axum으로 선택. 이유: Tower 기반 middleware/test ecosystem, Tokio 친화성, WebSocket 경로 통합, handler를 얇게 유지하기 쉽다.

## 1. 목적

Controller는 Fleet의 control plane이다. 이 task에서는 아직 agent WebSocket 연결을 완성하지 않고, Controller가 안전하게 시작되고 admin/enrollment token을 다룰 수 있는 API 기반을 만든다.

기능 묶음:

- Controller bootstrap과 identity
- Admin bootstrap token
- Enrollment token API

## 2. 선행 조건

- [x] Task 001 완료
- [x] Task 002 완료
- [x] Task 003 완료
- [x] SQLite migration과 controller identity 저장소가 준비되어 있다.

## 3. 기능 묶음 A. Controller bootstrap과 identity

### 작업

- [x] `fleet-controller` library bootstrap 작성. 실행은 단일 `sponzey controller start` subcommand로 통합
- [x] Axum 또는 Actix Web 중 하나를 선택한다. Controller 전환 대상은 Axum으로 고정한다.
- [x] bind address, data dir, database URL을 typed `Settings`로 연결한다.
- [x] `controller init` 시 controller signing key pair를 생성한다.
- [x] controller private key 저장 권한을 검증한다.
- [x] controller public key fingerprint를 출력한다.
- [x] health endpoint를 만든다.
- [x] Product 로그로 startup event를 기록한다.
- [x] graceful shutdown을 구현한다.

### 테스트/검증

- [x] settings validation test
- [x] controller identity created once test
- [x] controller private key permission test
- [x] health endpoint integration test
- [x] startup with temp SQLite test
- [x] shutdown smoke test

### 완료 기준

- [x] `sponzey controller start --db sqlite://...`로 서버가 뜬다.
- [x] controller는 시작 중 env var를 직접 읽지 않는다.
- [x] controller signing key 없이는 task dispatch를 시작할 수 없다.
- [x] private key 원문은 Product 로그에 남지 않는다.

## 4. 기능 묶음 B. Admin bootstrap token

### 작업

- [x] local bootstrap admin token 생성 방식을 정의한다.
- [x] token은 생성 시 1회만 출력한다.
- [x] token hash를 저장한다.
- [x] CLI request authentication header를 정의한다.
- [x] MVP는 full RBAC 없이 admin token만 사용한다.
- [x] token redaction을 logging/audit path에 적용한다.
- [x] missing/invalid token 응답을 명확히 한다.

### TDD

- [x] token hash verification
- [x] invalid token rejected
- [x] missing token rejected
- [x] token redaction in logs
- [x] protected API requires admin token

### 완료 기준

- [x] protected API는 admin token 없이 접근할 수 없다.
- [x] Product 로그에 token 원문이 남지 않는다.
- [x] CLI가 admin token을 명시 인자로 controller job API에 전달할 수 있다.

## 5. 기능 묶음 C. Enrollment token API

### 작업

- [x] `POST /api/enrollment-tokens`
- [x] `GET /api/enrollment-tokens`
- [x] `DELETE /api/enrollment-tokens/{id}`
- [x] token TTL 구현
- [x] max uses 구현
- [x] default labels 구현
- [x] token 원문은 생성 시 1회만 반환한다.
- [x] enrollment response에는 controller public key/fingerprint가 포함된다.
- [x] enrollment token은 agent identity 등록에만 사용한다.
- [x] enrollment token을 WebSocket task channel 인증에 재사용하지 않는다.

### TDD

- [x] create token test
- [x] expired token rejected
- [x] revoked token rejected
- [x] max uses exceeded test
- [x] token raw value not stored test
- [x] enrollment response includes controller fingerprint test
- [x] enrollment token cannot authenticate websocket task channel test

### 완료 기준

- [x] `sponzey enroll-token create`가 동작한다.
- [x] token 생성/폐기는 audit에 남는다.
- [x] token은 enrollment 이후 agent config에 남지 않는다.

## 6. API 문서

- [x] health endpoint 문서
- [x] admin token 사용 방식 문서
- [x] enrollment token lifecycle 문서
- [x] controller identity와 public key fingerprint 문서

## 7. 완료 전 체크

- [x] admin token verification과 REST 조회/수정 handler는 application use case adapter를 경유한다.
- [x] command job handler는 DB를 직접 조작하지 않고 use case를 호출한다.
- [x] token 원문이 DB/log/audit에 저장되지 않는다.
- [x] controller private key 권한 검증이 있다.
- [x] runtime config mutation endpoint를 만들지 않았다.
