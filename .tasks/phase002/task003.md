# Task 003. Controller Identity Pinning과 Trust Model

상위 계획: `Phase 002`
목표: TLS certificate trust와 controller signing identity를 분리하고, agent가 controller identity 변경을 안전하게 감지하도록 만든다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- Controller signing identity pinning
- TLS certificate trust policy
- Rotation 준비

## 1. 배경

TLS는 transport 보안이고, controller signing key는 task/enrollment 신뢰의 제품 identity다. 두 개를 섞으면 certificate 교체 때 agent trust가 깨지거나, 반대로 signing key 변경을 TLS 인증서 변경처럼 가볍게 처리하는 위험이 생긴다.

## 2. 작업

- [x] controller identity response에 signing public key fingerprint와 TLS endpoint metadata를 명확히 분리한다.
- [x] agent config에 pinned controller signing fingerprint를 저장한다.
- [x] agent는 heartbeat/task channel 시작 전 controller signing fingerprint를 검증한다.
- [ ] TLS certificate fingerprint pinning을 optional policy로 설계하되, signing identity pinning과 혼동하지 않는다.
- [x] controller signing key rotation은 Phase 2에서 자동화하지 않더라도, 상태와 문서에 safe migration path를 둔다.
- [x] pin mismatch 오류 메시지는 재등록이 필요한지, 공격 가능성이 있는지 구분해서 설명한다.

## 3. 테스트

- [x] 같은 controller identity면 reconnect가 성공한다.
- [ ] TLS certificate만 교체되고 signing identity가 같으면 정책에 따라 허용된다.
- [x] signing identity가 바뀌면 agent가 거부한다.
- [ ] pin mismatch는 security audit event로 남는다.
- [ ] Product 로그에 private key나 token이 나오지 않는다.

## 4. 완료 기준

- [x] agent가 controller identity 변경을 조용히 받아들이지 않는다.
- [x] TLS 인증서 교체와 controller signing key 교체가 문서에서 분리되어 있다.
- [x] 운영자가 pin mismatch 원인을 판단할 수 있는 메시지가 있다.
