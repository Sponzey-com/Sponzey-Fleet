# Task 008. Web Admin UI 운영 표면 확장

상위 계획: `Phase 002`
목표: Web Admin UI를 원격 운영자가 agent 등록, 상태 확인, approval, audit를 수행할 수 있는 얇은 운영 표면으로 확장한다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- Enrollment token UI
- Agent detail/health UI
- Approval/audit UI

## 1. 작업

- [x] enrollment token create/list/revoke 화면을 만든다.
- [x] token 원문은 생성 직후 1회만 표시한다.
- [x] agent detail 화면에 facts/metrics/drift/audit summary를 표시한다.
- [ ] agent labels update는 audit와 함께 API를 통해 수행한다.
- [ ] approval queue 화면을 만든다.
- [ ] high-risk job approve/reject confirmation UI를 만든다.
- [ ] audit list에 filter와 target deep link를 추가한다.

## 2. UI 원칙

- UI는 domain rule을 복제하지 않는다.
- 권한 판단은 controller application layer에서 한다.
- secret은 localStorage에 장기 저장하지 않는다.
- token 원문을 다시 조회하는 기능을 만들지 않는다.

## 3. 테스트

- [x] API client type check
- [x] enrollment token creation rendering test
- [x] token secret one-time display test
- [x] agent list/detail rendering smoke
- [ ] approval confirmation test
- [ ] forbidden response display test

## 4. 완료 기준

- [x] CLI 없이도 remote agent 등록 준비를 할 수 있다.
- [ ] 운영자가 Web Admin UI에서 agent health와 job approval을 처리할 수 있다.
- [x] UI가 runtime config editor로 변질되지 않는다.
