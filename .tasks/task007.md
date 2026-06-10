# Task 007. Approval 기반 Remote Execution

상위 계획: `Phase 002`
목표: 원격 agent에서 위험 작업을 실행할 때 approval, timeout, cancel, output limit, privilege policy를 적용한다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- Approval queue
- Execution boundary
- Privilege escalation policy

## 1. 작업

- [x] job 생성 시 risk classification을 domain rule로 계산한다.
- [ ] high-risk job은 `PendingApproval` 상태로 저장한다.
- [ ] approve/reject API와 CLI를 추가한다.
- [x] approved job만 signed task envelope로 dispatch한다.
- [x] job cancel과 timeout을 agent runner에 반영한다.
- [x] stdout/stderr output size limit을 명확히 적용한다.
- [x] `sudo`/`su` 같은 privilege escalation은 primitive policy로 다룬다.
- [x] controller가 임의 shell 문자열로 `su`를 대신 수행하는 설계를 금지한다.
- [x] root-required task는 explicit approval과 audit를 요구한다.

## 2. 테스트

- [ ] low-risk command는 바로 dispatch된다.
- [x] high-risk command는 approval 전 dispatch되지 않는다.
- [ ] rejected job은 agent에 전달되지 않는다.
- [x] approved job은 signed envelope로 전달된다.
- [x] timeout 시 child process가 종료된다.
- [x] output limit 초과 시 truncate event가 남는다.
- [x] privilege policy 위반은 실행 전 거부된다.

## 3. 완료 기준

- [x] 원격 서버에서 위험 명령이 approval 없이 실행되지 않는다.
- [x] cancel/timeout/output limit이 실제 runner boundary에 적용된다.
- [x] privilege escalation은 감사 가능한 정책으로만 가능하다.
