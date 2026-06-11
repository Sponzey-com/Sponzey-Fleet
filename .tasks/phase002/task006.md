# Task 006. Multi-Agent Inventory와 Health

상위 계획: `Phase 002`
목표: 여러 원격 agent를 안정적으로 관리하기 위해 inventory, health, selector, facts/metrics 조회를 강화한다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- Agent health state machine
- Facts/metrics indexing
- Selector targeting UX

## 1. 작업

- [x] online/offline/degraded/disabled 상태 전이를 domain state machine으로 정리한다.
- [ ] heartbeat timeout과 last seen 기준을 settings로 명확히 둔다.
- [x] facts snapshot의 주요 필드를 inventory list에서 빠르게 볼 수 있게 저장한다.
- [ ] labels, hostname, OS, arch 기준 selector를 확장한다.
- [x] agent detail API에 facts, metrics, drift summary를 묶어서 제공한다.
- [x] Web Admin UI agent list에 health, last seen age, labels, OS를 표시한다.

## 2. 테스트

- [x] state transition domain tests
- [ ] heartbeat timeout application tests
- [x] disabled agent는 dispatch 대상에서 제외된다.
- [x] selector matching tests
- [x] repository index/query tests
- [x] Web Admin UI rendering smoke

## 3. 완료 기준

- [x] agent가 여러 대 붙어도 운영자가 상태를 빠르게 판단할 수 있다.
- [ ] selector가 실제 원격 실행 대상을 예측 가능하게 고른다.
- [ ] offline/degraded 상태가 audit와 Product log에 남는다.
