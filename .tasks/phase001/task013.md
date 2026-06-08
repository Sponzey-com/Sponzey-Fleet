# Task 013. MVP Hardening과 Release Readiness

상위 계획: `M12. MVP Hardening`  
목표: 보안, 설정, 로그, smoke test, 문서를 공개 가능한 MVP 수준으로 마무리한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] `docs/security-checklist.md` 추가
- [x] `docs/configuration.md` 추가
- [x] `docs/logging.md` 추가
- [x] cargo fmt/check/test/clippy 통과
- [x] local smoke script 통과
- [x] SQLite migration/repository/audit unit test 추가
- [x] WebSocket heartbeat smoke path 추가
- [x] `scripts/hardening_audit.sh` code-search hardening gate 추가 및 통과
- [x] retention 기본값 문서 추가
- [x] MVP release notes 추가
- [x] service install 방향 문서 추가
- [x] npm wrapper demo smoke 추가 및 통과
- [x] dev-insecure-loopback Product 로그와 Security audit 보강
- [x] Web Admin UI static build/test 추가 및 통과
- [x] runbook validation-only apply smoke 추가 및 통과
- [x] signed runbook dispatch API와 agent-side execution path 추가
- [x] retention cleanup dry-run smoke 추가 및 통과
- [x] 실제 REST/WebSocket/SQLite/Web UI 기준 hardening smoke와 regression test 통과
- [x] MVP smoke에 signed runbook dispatch 실패 경로 검증 추가 및 통과
- [x] 통합 release readiness gate script 추가
- [x] release readiness gate 통과. 단, destructive Linux/manual checks는 `--include-manual` 경로로 분리

## 1. 목적

기능이 동작하는 것과 공개 가능한 MVP는 다르다. 이 task는 원격 명령 실행 제품으로서 최소 신뢰 경계를 검증하고, fresh checkout에서 MVP demo가 재현되도록 만든다.

기능 묶음:

- Security review
- Configuration/logging review
- End-to-end smoke suite와 release docs

## 2. 선행 조건

- [x] Task 001~012 MVP 구현 범위 완료
- [x] local controller/agent command run이 가능하다.
- [x] Web Admin UI build가 가능하다.

## 3. 기능 묶음 A. Security review

### 작업

- [x] token redaction review
- [x] command output logging review
- [x] audit coverage review
- [x] local config permission review
- [x] dangerous task classification review
- [x] timeout/cancel review
- [x] controller signing key review
- [x] agent identity proof review
- [x] signed task envelope verification review
- [x] insecure loopback-only mode review
- [x] high-risk confirmation review

### 검증

- [x] Product log에 secret이 없는지 fixture test
- [x] FieldDebug log에도 secret redaction이 적용되는지 test
- [x] command output이 app log에 섞이지 않는지 test
- [x] unsigned task rejection test
- [x] invalid signature rejection test
- [x] expired task rejection test
- [x] replayed task rejection test
- [x] target mismatch task rejection test
- [x] non-loopback insecure URL rejection test
- [x] high-risk command without confirmation rejection test
- [x] audit coverage checklist

### 완료 기준

- [x] MVP 보안 체크리스트가 통과한다.
- [x] security-sensitive failure는 audit에 남는다.
- [x] 원격 insecure transport는 불가능하다.

## 4. 기능 묶음 B. Configuration/logging review

### 작업

- [x] `std::env::var` 사용 위치 audit
- [x] `std::env::set_var` production code 금지 확인
- [x] settings bootstrap path 문서화
- [x] CLI arg/config file precedence 문서화
- [x] runtime config mutation endpoint 없음 확인
- [x] Product/FieldDebug/Development profile 문서화
- [x] job output storage와 application log 분리 확인
- [x] retention 기본값 문서화

### 검증

- [x] code search
- [x] settings tests
- [x] log profile tests
- [x] redaction tests
- [x] docs review

### 완료 기준

- [x] 외부 환경 상수는 bootstrap에서만 읽힌다.
- [x] 설정 변경은 restart 또는 명시적 command로만 가능하다.
- [x] 로그 3단계가 구현과 문서에 일치한다.

## 5. 기능 묶음 C. End-to-end smoke suite와 release docs

### 작업

- [x] local controller start smoke
- [x] local agent enroll smoke
- [x] heartbeat 확인 smoke
- [x] command run smoke
- [x] facts smoke
- [x] metrics smoke
- [x] logs smoke
- [x] drift check smoke
- [x] audit query smoke
- [x] Web Admin UI static serving smoke
- [x] `scripts/smoke_mvp.sh` 또는 동등 script 작성
- [x] `scripts/npm_demo_smoke.sh` 작성
- [x] root 권한 없는 CI smoke와 manual systemd smoke 분리
- [x] MVP release notes 작성

### 검증

- [x] fresh checkout에서 smoke 재현
- [x] root 권한 없이 가능한 smoke는 CI-friendly하게 동작
- [x] root/systemd 부분은 manual smoke로 분리
- [x] docs command가 실제 CLI와 일치

### 완료 기준

- [x] fresh checkout에서 MVP smoke가 재현된다.
- [x] MVP demo script가 문서화되어 있다.
- [x] release notes에 known limitations가 포함되어 있다.

## 6. MVP Definition of Done 체크

- [x] `cargo fmt` 통과
- [x] `cargo clippy --workspace --all-targets` 통과
- [x] `cargo test --workspace` 통과
- [x] Web Admin UI build 통과
- [x] local smoke test 통과
- [x] docs updated
- [x] Product 로그 기본값 확인
- [x] FieldDebug 로그에서 secret redaction 확인
- [x] Development 로그가 production 기본값이 아님을 확인
- [x] env var 중간 변경 코드 없음
- [x] runtime config mutation endpoint 없음
- [x] non-loopback insecure transport 거부
- [x] controller public key pinning 동작
- [x] authenticated agent만 WebSocket task channel 사용 가능
- [x] unsigned/invalid/expired/replayed task 거부
- [x] high-risk command는 explicit confirmation 없이는 실행 불가
- [x] audit 이벤트 누락 없음
- [x] command output과 application log 분리
- [x] SQLite migration repeatable
- [x] npm wrapper로 `sponzey --help` 가능
- [x] npm wrapper로 local demo 가능
- [x] controller/agent/service install 문서화

## 7. 완료 전 체크

- [x] 보안 검토가 hardening 문서에만 있고 테스트에 없는 상태가 아니다.
- [x] MVP 제외 범위가 release notes에 명확하다.
- [x] 공개 demo가 insecure remote 사용을 권장하지 않는다.
- [x] 운영자가 실패 원인을 알 수 있는 Product 로그가 남는다.