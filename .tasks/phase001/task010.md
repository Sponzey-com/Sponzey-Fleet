# Task 010. Drift Detection

상위 계획: `M9. Drift Detection`  
목표: policy 기반 drift check와 nginx running demo를 구현한다.  
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 진행 메모

- [x] DriftReport/DriftStatus domain type 구현
- [x] `examples/policies/nginx-running.yml` 추가
- [x] `sponzey drift check --policy` local skeleton 구현
- [x] MVP policy domain model과 제한된 YAML validator 구현
- [x] service/package/file checksum check model parse test 추가
- [x] agent-side check engine 구현
- [x] agent-side service/package/file checksum check 구현
- [x] controller drift check job dispatch API 구현
- [x] SQLite drift check pending assignment 저장/조회 구현

## 1. 목적

Sponzey Fleet의 핵심 차별점은 agent가 상시 상태를 알고 drift를 감지할 수 있다는 점이다. MVP에서는 service/package/file checksum 중심의 작은 policy부터 구현한다.

기능 묶음:

- Policy model
- Agent-side check
- Drift CLI/API/UI 연결

## 2. 선행 조건

- [x] Task 007 완료
- [ ] Task 008 완료
- [x] service/package/file primitive 또는 check adapter가 준비되어 있다.

## 3. 기능 묶음 A. Policy model

### 작업

- [x] `Policy` domain model 정의
- [x] policy selector 정의
- [x] service state check 정의
- [x] package present check 정의
- [x] file checksum check 정의
- [x] remediation block은 MVP에서 manual proposal까지만 허용
- [x] remediation without approval은 validation에서 거부
- [x] policy YAML parser 구현
- [x] sample `examples/policies/nginx-running.yml` 작성

### TDD

- [x] valid policy parse
- [x] invalid selector rejected
- [x] unsupported check rejected
- [x] remediation without approval rejected
- [x] service check parse test
- [x] file checksum check parse test

### 완료 기준

- [x] policy YAML을 validation할 수 있다.
- [x] policy가 자동 remediation을 즉시 실행하지 않는다.
- [x] invalid policy는 실행 전 거부된다.

## 4. 기능 묶음 B. Agent-side check

### 작업

- [x] service running check 구현
- [x] package present check 구현
- [x] file checksum check 구현
- [x] expected/actual report 구조화
- [x] drift status 정의

  - [x] `compliant`
  - [x] `drifted`
  - [x] `unknown`
- [x] check failure를 unknown과 failed execution으로 구분한다.
- [x] drift report protocol message 구현

### TDD/검증

- [x] service running fixture
- [x] service stopped fixture
- [x] package present fixture
- [x] package missing fixture
- [x] file checksum mismatch
- [x] unknown check behavior
- [x] expected/actual serialization test

### 완료 기준

- [x] nginx stopped 상태를 drift로 감지한다.
- [x] expected/actual diff가 구조화되어 저장된다.
- [x] unknown은 drifted와 구분된다.

## 5. 기능 묶음 C. Drift CLI/API/UI 연결

### 작업

- [x] `sponzey drift check --policy nginx-running` 구현
- [x] drift check job dispatch 구현
- [x] drift report storage 구현
- [x] drift result audit 구현
- [x] drift report API 구현
- [x] UI drift diff rendering 구현
- [x] remediation 실행과 drift 감지를 분리한다.
- [x] protocol `drift_check` task wire type 구현
- [x] agent-side signed drift check task 처리 구현
- [x] `/api/jobs/drift-check` API 구현

### TDD/검증

- [x] CLI parse test
- [x] drift report API test
- [x] drift report storage test
- [x] e2e drift smoke test
- [x] UI drift diff render test

### 완료 기준

- [x] expected/actual diff가 CLI에 보인다.
- [x] expected/actual diff가 UI에 보인다.
- [x] drift 감지는 remediation 실행과 분리된다.
- [x] drift event는 audit에 남는다.

## 6. 완료 전 체크

- [x] auto remediation을 MVP에 몰래 넣지 않았다.
- [x] drift check는 signed task envelope를 통해 실행된다.
- [x] check 결과와 command output log가 분리되어 있다.