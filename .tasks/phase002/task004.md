# Task 004. Enrollment Token 제품화

상위 계획: `Phase 002`
목표: enrollment token을 원격 운영에 맞게 scope, expiry, single-use, revoke, audit를 갖춘 제품 기능으로 만든다.
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 기능 묶음

- Token lifecycle
- Bootstrap command generation
- Audit와 UI/API 연동

## 1. 작업

- [x] token scope를 정의한다. 예: labels, max uses, expires at, allowed agent name prefix.
- [x] token create/list/revoke CLI를 정리한다.
- [x] token은 생성 시 1회만 원문 출력한다.
- [x] 저장소에는 token hash만 저장한다.
- [x] token 사용 시 remaining uses가 감소하고 0이면 폐기된다.
- [x] expired/revoked/used token은 명확한 오류를 반환한다.
- [x] controller가 agent install/init one-line command를 생성한다.
- [x] Web Admin UI에서 token 생성과 revoke를 지원한다.

## 2. 테스트

- [x] token 원문은 create output 외 로그에 남지 않는다.
- [x] hash 저장을 repository test로 확인한다.
- [x] expired token 거부
- [x] revoked token 거부
- [x] max uses 초과 거부
- [x] label scope가 enrollment agent labels에 반영된다.
- [x] token create/use/revoke audit event가 생성된다.

## 3. 완료 기준

- [x] 다른 노트북 등록에 필요한 command를 controller에서 안전하게 만들 수 있다.
- [x] token lifecycle을 CLI/API/UI에서 확인할 수 있다.
- [x] token 오남용이 audit에 남는다.
