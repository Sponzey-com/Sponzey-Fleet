# Task 011. Web Admin UI

상위 계획: `M10. Web Admin UI`  
목표: Controller가 정적 파일로 서빙하는 얇은 Web Admin UI를 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] `web-admin/index.html` placeholder 추가
- [x] `web-admin/styles.css` placeholder 추가
- [x] Controller `/admin` static serving 구현
- [x] static asset serving integration test 추가
- [x] dependency-free static export build script 추가
- [x] Web Admin UI smoke test 추가
- [x] dependency-free API client와 UI render tests 구현
- [x] polling 기반 output/job 실행 실제 화면 구현
- [x] agent/facts/metrics/drift/audit 실제 화면 구현
- [x] recent job history API/UI 구현
- [x] `checkJs` 기반 `tsconfig.json` 추가
- [x] shared API schema와 dependency-free API client 분리
- [x] dependency-free API client type surface check 명령 추가

## 1. 목적

Web Admin UI는 별도 제품이 아니라 운영자가 agent 상태, job output, facts/metrics, drift, audit을 빠르게 확인하고 최소 실행을 수행하는 얇은 admin surface다.

기능 묶음:

- Web Admin UI scaffold
- Agents/Jobs screens
- Facts/Metrics/Drift/Audit screens

## 2. 선행 조건

- [x] Task 006 완료
- [x] Task 007 완료
- [x] Task 010 완료 또는 drift API skeleton 존재

## 3. 기능 묶음 A. Web Admin UI scaffold

### 작업

- [x] dependency-free static export를 선택한다.
- [x] `web-admin/` 프로젝트를 생성한다.
- [x] TypeScript 설정을 작성한다. `checkJs` 기반 `tsconfig.json`을 추가했다.
- [x] generated API client 또는 shared schema 기반 client를 준비한다. MVP에서는 shared schema 기반 dependency-free client를 사용한다.
- [x] Controller가 `/admin`에서 static asset을 서빙한다.
- [x] 별도 Node.js web server를 운영하지 않는다.
- [x] auth token 입력/저장 정책을 MVP에서 제외한다.
- [x] 브라우저 localStorage 장기 token 저장을 피한다.

### TDD/검증

- [x] UI build
- [x] API client type check. MVP에서는 dependency-free API schema/client surface check로 검증한다.
- [x] static asset serving integration test
- [x] auth token missing state render는 API client 도입 전 제외로 고정

### 완료 기준

- [x] controller가 `/admin`에서 UI를 제공한다.
- [x] 별도 Node.js web server가 필요 없다.
- [x] UI는 domain rule을 직접 구현하지 않는다.

## 4. 기능 묶음 B. Agents/Jobs screens

### 작업

- [x] agents table 구현
- [x] agent status 표시
- [x] agent detail 화면 구현
- [x] run command form 구현
- [x] high-risk command confirmation UI 구현
- [x] job history 구현
- [x] job detail 구현
- [x] live output viewer 구현
- [x] output redaction 표시 정책을 반영한다.

### TDD/검증

- [x] agents table render test
- [x] run command form validation test
- [x] high-risk confirmation required test
- [x] live output component test
- [x] job detail render test

### 완료 기준

- [x] 브라우저에서 agent를 선택해 command를 실행하고 output을 본다.
- [x] high-risk command는 confirmation 없이 실행되지 않는다.
- [x] UI authorization은 controller 응답을 따른다.

## 5. 기능 묶음 C. Facts/Metrics/Drift/Audit screens

### 작업

- [x] facts panel 구현
- [x] metrics snapshot panel 구현
- [x] drift report diff 구현
- [x] audit event list 구현
- [x] product/field debug log와 audit의 차이를 UI 도움말에 짧게 설명한다.
- [x] secret 원문 표시를 기본 금지한다.
- [x] forbidden response를 명확하게 표시한다.

### TDD/검증

- [x] facts render test
- [x] metrics render test
- [x] drift diff render test
- [x] audit list render test
- [x] forbidden response render test

### 완료 기준

- [x] MVP demo를 UI만으로 설명할 수 있다.
- [x] UI에 domain rule이 중복되지 않는다.
- [x] UI가 설정 편집기나 workflow designer로 확장되지 않았다.

## 6. 완료 전 체크

- [x] Controller static serving만 사용한다.
- [x] UI는 API 호출과 렌더링에 집중한다.
- [x] Web Admin UI가 런타임 환경 설정 변경 기능을 제공하지 않는다.
- [x] secret/token 원문이 화면에 표시되지 않는다.
