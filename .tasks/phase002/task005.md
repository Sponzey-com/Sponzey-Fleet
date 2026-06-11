# Task 005. Agent Production Service Lifecycle

상위 계획: `Phase 002`
목표: agent를 production service로 설치, 시작, 중지, 상태 확인, 로그 확인, 제거할 수 있게 한다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- systemd service lifecycle
- data directory/permission 정책
- uninstall/cleanup 설계

## 1. 작업

- [x] `sponzey agent install-service`를 HTTPS remote 설정 기준으로 갱신한다.
- [x] service unit에 `sponzey agent start --data-dir ...`만 남기고 secret을 command line에 넣지 않는다.
- [ ] `/var/lib/sponzey-fleet` 같은 production data dir permission을 검증한다.
- [x] private key/config permission이 group/other readable이면 start를 거부한다.
- [x] `sponzey agent service status` 또는 기존 `start-service` 계열 UX를 정리한다.
- [x] `sponzey agent uninstall-service --dry-run` 설계를 추가한다.
- [x] journald log 확인 command를 문서화한다.

## 2. 테스트

- [x] service unit rendering test
- [x] shell quoting test
- [x] invalid user/group rejection test
- [x] private key permission rejection test
- [x] dry-run output test
- [x] Linux/root 필요 작업은 manual smoke로 분리

## 3. 완료 기준

- [ ] agent 노트북/서버에서 재부팅 후 agent가 자동으로 다시 붙는다.
- [x] service command line에 token/private key가 노출되지 않는다.
- [x] 설치/제거가 dry-run으로 확인 가능하다.
