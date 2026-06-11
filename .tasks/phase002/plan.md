# Sponzey Fleet Phase 002 계획

상위 방향: MVP 이후 제품화 Beta
목표: SSH tunnel 없이 다른 노트북/서버 agent가 안전하게 controller에 붙고, 원격 운영 자동화 제품으로 사용할 수 있는 최소 production path를 만든다.
상태: `[ ] 대기` `[ ] 계획 수립` `[x] 진행 중` `[ ] 완료`

## 0. Phase 002 정의

Phase 001은 로컬 MVP였다. Phase 002는 원격 agent를 제품스럽게 받기 위한 첫 단계다.

핵심 전환:

- `http://127.0.0.1` loopback demo 중심에서 `https://controller.example.com` 원격 운영 중심으로 전환한다.
- SSH tunnel은 개발용 임시 우회로만 남긴다.
- controller와 agent가 TLS, controller identity pinning, agent identity proof, signed task envelope를 모두 유지한 상태로 원격 통신한다.
- npm 설치 UX는 유지하되, production 설치는 systemd, 고정 data directory, log/audit 확인, service lifecycle로 이어진다.
- Web Admin UI는 remote enrollment token, agent health, approval, audit 확인까지 운영 표면으로 확장한다.

## 1. Phase 002에서 하지 않을 것

아래는 명시적으로 거부한다.

- 원격 `http://192.168.x.x` 허용 플래그
- `--allow-insecure-remote`, `--disable-tls-verification` 같은 우회 기능
- agent가 unsigned task를 실행하도록 만드는 호환 옵션
- controller runtime env를 UI/API로 바꾸는 설정 편집기
- Ansible full compatibility
- multi-tenant enterprise 기능
- Kubernetes-only 배포
- root shell을 무제한 primitive로 여는 기능

개발 편의는 SSH tunnel이나 loopback demo로 해결하고, 제품 경로는 HTTPS/TLS로만 간다.

## 2. Phase 002 성공 기준

- 다른 노트북/서버에서 `sponzey agent init --url https://...`로 등록할 수 있다.
- agent는 controller TLS endpoint에 outbound로 연결한다.
- agent는 enrollment 시 controller identity를 pinning하고 이후 변경을 거부한다.
- controller는 agent identity proof 이후에만 task channel을 연다.
- enrollment token은 scope, expiry, single-use, revoke가 명확하다.
- controller external URL과 bind address가 분리된다.
- Web Admin UI에서 enrollment token을 만들고 revoke할 수 있다.
- Web Admin UI에서 agent online/offline, labels, facts, metrics, last seen을 확인한다.
- high-risk command는 approval 없이는 원격 agent에서 실행되지 않는다.
- systemd install/start/status/log 확인 경로가 문서와 smoke로 검증된다.
- release artifact는 Linux glibc 호환성 또는 musl static 방향을 결정하고 검증한다.

## 3. 아키텍처 원칙

기존 `AGENTS.md` 원칙을 유지한다.

- Domain/Application layer는 TLS, filesystem, HTTP 구현 세부사항을 직접 알지 않는다.
- TLS, certificate, network transport는 infrastructure/interface boundary에 둔다.
- Settings는 bootstrap에서 한 번 만들고 불변으로 전달한다.
- runtime 중 env var를 읽거나 바꾸지 않는다.
- Product, FieldDebug, Development log profile을 유지한다.
- secret/token/private key는 Product/FieldDebug 로그에 원문으로 나오지 않는다.

## 4. 작업 목록

| Task     | 제목                                       | 핵심 기능 묶음                                                             |
| -------- | ---------------------------------------- | -------------------------------------------------------------------- |
| Task 001 | Production Settings와 Remote URL 경계       | bind/external URL 분리, HTTPS URL validation, CLI UX                   |
| Task 002 | TLS HTTP/WebSocket Transport             | controller HTTPS listener, agent HTTPS/WSS client, HTTP warning 유지   |
| Task 003 | Controller Identity Pinning과 Trust Model | TLS와 controller signing identity 분리, pinning 검증, rotation 준비         |
| Task 004 | Enrollment Token 제품화                     | scope/expiry/single-use/revoke, bootstrap command, audit             |
| Task 005 | Agent Production Service Lifecycle       | systemd install/start/status/logs, data dir permission, uninstall 설계 |
| Task 006 | Multi-Agent Inventory와 Health            | heartbeat scale, online/offline transition, selector/facts indexing  |
| Task 007 | Approval 기반 Remote Execution             | approval queue, cancellation/timeout/output limit, privilege policy  |
| Task 008 | Web Admin UI 운영 표면 확장                    | enrollment UI, agent detail, approvals/audit filters                 |
| Task 009 | Packaging/Release Hardening              | Linux compatibility, npm/standalone artifacts, release smoke         |

현재 진행 상태:

- Task 001: 완료. bind/external URL 경계, remote HTTPS URL validation, CLI help, README 정합성을 반영했다.
- Task 002: 완료. built-in HTTPS listener, cert/key bootstrap 검증, agent HTTPS/WSS, remote TLS smoke를 반영했다.
- Task 003: 진행 중. controller signing identity와 TLS endpoint metadata는 분리했고 agent pinning도 검증하지만, optional TLS certificate pinning policy와 pin mismatch audit는 남아 있다.
- Task 004: 완료. enrollment token scope/expiry/single-use/revoke, CLI/API/UI lifecycle, create/use/revoke audit를 반영했다.
- Task 005: 진행 중. systemd install/start/uninstall dry-run, secret-free service command, secure config permission 검증은 반영했지만 production data dir permission 검증은 남아 있다.
- Task 006: 진행 중. agent status state, facts/metrics/drift API, inventory health summary, Web Admin rendering은 반영했지만 heartbeat timeout setting과 hostname/OS/arch selector 확장은 남아 있다.
- Task 007: 진행 중. high-risk confirmation, signed task envelope, timeout/output/privilege boundary는 반영했지만 approval queue와 approve/reject workflow는 남아 있다.
- Task 008: 진행 중. enrollment token UI, agent health/detail, audit surface는 반영했지만 approval queue, label update UI, audit filter/deep link는 남아 있다.
- Task 009: 진행 중. glibc baseline check, remote TLS gate, standalone artifacts, checksums, release notes template은 반영했지만 musl decision과 workflow dry-run 검증은 남아 있다.

## 5. Phase 002 진행 순서

1. Task 001로 설정 경계와 CLI UX를 먼저 고정한다.
2. Task 002와 Task 003으로 HTTPS/WSS transport와 identity pinning을 만든다.
3. Task 004로 remote enrollment를 제품 수준으로 올린다.
4. Task 005와 Task 006으로 실제 서버 여러 대 운영 경로를 만든다.
5. Task 007과 Task 008로 원격 실행과 Web Admin 운영 표면을 확장한다.
6. Task 009로 배포, smoke, 문서를 묶어 beta release 기준을 만든다.

## 6. 공통 검증 게이트

각 task 완료 전 최소 확인:

- [x] `cargo fmt`
- [x] `cargo check -p fleet-cli`
- [x] 해당 crate unit/application tests
- [x] `cargo test -p fleet-cli` 또는 변경 영향권 테스트
- [x] `npm test --workspace @sponzey/fleet` if npm metadata changed
- [x] smoke script 추가 또는 기존 smoke 갱신
- [x] 문서 예시와 실제 CLI help 일치
- [x] token/secret redaction 확인
- [x] HTTP transport가 테스트 전용으로 안내되고 warning/audit가 남는지 확인

## 7. Phase 002 Definition of Done

- [x] loopback demo는 그대로 동작한다.
- [x] 원격 agent는 HTTPS/WSS로 등록 및 실행된다.
- [x] insecure remote HTTP는 계속 실패한다.
- [x] controller external URL 설정이 명확하다.
- [x] controller certificate/trust 모델이 문서화되어 있다.
- [x] enrollment token lifecycle이 UI/CLI/API에서 일관된다.
- [x] systemd 기반 controller/agent 서비스 경로가 검증된다.
- [ ] remote command execution은 approval/audit/output limit을 만족한다.
- [ ] Web Admin UI는 remote 운영자가 agent와 job 상태를 판단할 수 있을 만큼 충분하다.
- [x] npm release와 standalone binary release 검증이 자동화되어 있다.