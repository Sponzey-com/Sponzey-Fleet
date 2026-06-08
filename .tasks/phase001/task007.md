# Task 007. Inventory와 Facts

상위 계획: `M6. Inventory와 Facts`  
목표: agent facts 수집, label selector dispatch, agent detail API를 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] selector domain/application 구현
- [x] `sponzey facts` 로컬 facts skeleton 구현
- [x] `sponzey agents list` 로컬 파일 기반 skeleton 구현
- [x] `GET /api/agents` REST API 구현
- [x] `GET /api/agents/{id}` REST API 구현
- [x] `PATCH /api/agents/{id}/labels` REST API 구현
- [x] label 변경 audit 구현
- [x] label validation error response 구현
- [x] command job REST API에 label/agent selector 연결
- [x] 최소 OS/arch/family facts snapshot protocol/storage 구현
- [x] latest facts API 구현
- [x] disabled agent는 selector dispatch target에서 제외하는 정책 구현
- [x] offline agent는 큐잉 가능한 dispatch target으로 유지하는 정책 구현
- [x] selector result count를 FieldDebug 로그와 job audit metadata에 기록
- [x] disabled selector 제외 API regression test 추가
- [x] OS/arch/runtime/CPU/hostname/memory/network facts collector 기본 구현
- [x] CLI와 agent snapshot이 동일한 local facts collector 사용
- [x] disk usage collector 구현
- [x] degraded signal 구현
- [x] Web Admin UI facts/agent detail 화면 연결
- [x] agent list/detail/label/facts API를 application use case adapter 경유로 전환

## 1. 목적

Fleet 운영의 기본은 “어떤 서버가 있고, 어떤 상태이며, 어떤 대상에 작업할 것인가”를 명확히 아는 것이다. 이 task는 facts와 inventory selector를 job dispatch에 연결한다.

기능 묶음:

- Facts collector
- Inventory selector application
- Agent detail API

## 2. 선행 조건

- [x] Task 005 완료
- [x] agent heartbeat와 repository가 동작한다.
- [x] selector domain model이 존재한다.

## 3. 기능 묶음 A. Facts collector

### 작업

- [x] OS facts 수집
- [x] CPU facts 수집
- [x] memory facts 수집
- [x] disk usage facts 수집
- [x] network facts 수집
- [x] hostname facts 수집
- [x] runtime family facts 수집
- [x] facts snapshot protocol message 구현
- [x] facts snapshot storage 연결
- [x] facts 수집 실패 시 degraded signal을 설계한다.

### TDD/검증

- [x] Linux fixture parser tests
- [x] facts serialization test
- [x] facts redaction test
- [x] missing `/proc` graceful failure test
- [x] degraded facts collection behavior test

### 완료 기준

- [x] `sponzey facts web-01`가 구조화된 facts를 출력한다.
- [x] facts 수집 실패는 agent degraded signal로 이어진다.
- [x] secret 또는 민감 정보가 facts에 섞이지 않는다.

## 4. 기능 묶음 B. Inventory selector application

### 작업

- [x] label selector를 job dispatch에 연결한다.
- [x] `agent:name` selector를 연결한다.
- [x] no target behavior를 정의한다.
- [x] disabled agent target policy를 정의한다.
- [x] offline agent target policy를 정의한다.
- [x] selector result count를 FieldDebug 로그와 audit metadata에 남긴다.
- [x] selector raw detail은 Product 로그에 과도하게 남기지 않는다.

### TDD

- [x] selector returns matching online agents
- [x] disabled agent excluded
- [x] offline agent behavior test
- [x] no target returns explicit error
- [x] multiple labels match test
- [x] selector result audit metadata test

### 완료 기준

- [x] `--selector role=web,env=prod`가 실제 job target을 만든다.
- [x] no target은 명확한 오류로 반환된다.
- [x] selector 결과는 count 중심으로 기록된다.

## 5. 기능 묶음 C. Agent detail API

### 작업

- [x] `GET /api/agents`
- [x] `GET /api/agents/{id}`
- [x] `PATCH /api/agents/{id}/labels`
- [x] latest facts endpoint
- [x] label 변경 audit
- [x] label validation error response
- [x] disabled/offline status response

### TDD/검증

- [x] list agents API test
- [x] get detail API test
- [x] label update validation test
- [x] label update audit test
- [x] latest facts API test
- [x] unauthorized label update rejected test

### 완료 기준

- [x] CLI와 API에서 agent/facts를 확인할 수 있다.
- [x] UI에서 agent/facts를 확인할 수 있다.
- [x] label 변경은 audit에 남는다.
- [x] invalid label은 domain validation을 통해 거부된다.

## 6. 완료 전 체크

- [x] facts collector가 Product 로그에 과도한 machine data를 남기지 않는다.
- [x] selector matching은 domain/application layer에 있다.
- [x] API handler는 repository를 직접 조작하지 않는다.
