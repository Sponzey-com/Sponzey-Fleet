# Task 002. TLS HTTP/WebSocket Transport

상위 계획: `Phase 002`
목표: controller HTTP API와 agent WebSocket을 HTTPS/WSS로 제공하고, loopback dev HTTP는 명시적 예외로만 유지한다.
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 기능 묶음

- Controller HTTPS listener
- Agent HTTPS/WSS client
- TLS 설정과 smoke test

## 1. 작업

- [x] Rust TLS stack을 선택한다. 기본 후보는 `rustls` 계열이다.
- [x] controller start에 `--tls-cert`, `--tls-key` 인자를 추가한다.
- [x] TLS certificate/key 파일 permission과 존재 여부를 bootstrap에서 검증한다.
- [x] `https://` API 요청과 `wss://` agent channel을 지원한다.
- [x] loopback `http://`는 `--dev-insecure-loopback`일 때만 유지한다.
- [x] non-loopback `http://`는 controller/agent 양쪽에서 계속 거부한다.
- [x] TLS handshake 실패를 Product 로그에는 안전하게, FieldDebug에는 진단 가능하게 남긴다.

진행 메모:

- 현재 완료된 HTTPS/WSS 항목은 agent/CLI client와 reverse proxy termination 경로 기준이다.
- controller process가 직접 TLS를 terminate하는 built-in HTTPS listener는 아직 구현하지 않았다.
- 다음 작업은 `--tls-cert`, `--tls-key` bootstrap validation과 self-signed loopback smoke다.

## 2. 설계 기준

- Domain/Application layer는 TLS를 알지 않는다.
- TLS 설정은 interface/infrastructure boundary에 둔다.
- 인증서 파일 경로는 bootstrap 인자로만 받는다.
- runtime 중 TLS cert path를 바꾸는 API는 만들지 않는다.

## 3. 테스트

- [x] self-signed certificate fixture로 HTTPS controller가 시작된다.
- [x] agent init이 `https://127.0.0.1:<port>`에 성공한다.
- [x] agent start가 `wss://127.0.0.1:<port>/api/agents/ws`로 heartbeat를 보낸다.
- [x] 잘못된 cert/key 조합은 bootstrap에서 실패한다.
- [x] cert permission이 너무 열려 있으면 경고 또는 실패 정책이 테스트된다.
- [x] non-loopback HTTP rejection regression test를 유지한다.

## 4. Smoke

- [x] `scripts/smoke_remote_tls_loopback.sh` 추가
- [x] temporary self-signed cert 생성
- [x] controller HTTPS start
- [x] enrollment token create
- [x] agent init over HTTPS
- [x] agent heartbeat over WSS
- [x] smoke 종료 시 임시 data directory 정리

## 5. 완료 기준

- [x] SSH tunnel 없이 HTTPS endpoint로 agent 등록이 가능하다.
- [x] WebSocket task channel도 TLS 위에서 동작한다.
- [x] insecure remote HTTP는 계속 불가능하다.
