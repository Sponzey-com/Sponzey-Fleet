# Task 009. Metrics와 Logs

상위 계획: `M8. Metrics와 Logs`  
목표: 운영 자동화 판단에 필요한 최소 metrics snapshot과 log tail streaming을 구현한다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 진행 메모

- [x] `sponzey metrics` 로컬 snapshot skeleton 구현
- [x] `sponzey logs --file` file tail skeleton 구현
- [x] log redaction helper 재사용
- [x] file log tail helper 구현
- [x] max line size truncation 구현
- [x] tail/redaction/file-not-found tests 추가
- [x] retention 기본값 문서화
- [x] agent-side metrics collector/protocol/storage 기본 구현
- [x] latest metrics API와 smoke 검증 추가
- [x] retention cleanup command/dry-run 구현
- [x] cleanup execution audit event 구현
- [x] file log follow/max-duration/cancel helper 구현
- [x] journald shortcut skeleton 구현

## 1. 목적

Sponzey Fleet는 Prometheus/Grafana 대체가 아니다. 이 task는 자동화 판단과 현장 확인에 필요한 최소 상태만 수집한다.

기능 묶음:

- Metrics snapshot
- Log tail streaming
- Retention policy

## 2. 선행 조건

- [x] Task 005 완료
- [x] Task 007의 facts/inventory API 범위 완료
- [x] authenticated agent channel과 facts storage가 동작한다.

## 3. 기능 묶음 A. Metrics snapshot

### 작업

- [x] CPU logical count 수집
- [x] memory total/available 수집
- [x] disk usage 수집
- [x] process count 수집
- [x] service status summary 수집
- [x] systemd failed units count 수집
- [x] metrics snapshot protocol message 구현
- [x] metrics snapshot storage 구현
- [x] `sponzey metrics web-01` CLI 구현

### TDD/검증

- [x] Linux `/proc` fixture parser tests
- [x] metrics serialization test
- [x] disk usage threshold formatting test
- [x] missing systemd graceful behavior test
- [x] metrics latest repository test

### 완료 기준

- [x] `sponzey metrics web-01`가 구조화된 local snapshot을 출력한다.
- [x] controller API가 최신 agent metrics snapshot을 출력한다.
- [x] UI에서 최근 snapshot을 볼 수 있다.
- [x] metrics는 time-series DB처럼 무제한 저장되지 않는다.

## 4. 기능 묶음 B. Log tail streaming

### 작업

- [x] file log tail 구현
- [x] journald adapter skeleton 구현
- [x] `sponzey logs web-01 --file /var/log/syslog` 구현
- [x] `sponzey logs nginx` shortcut 구현 가능 범위 정의
- [x] max line size 적용
- [x] max stream duration 적용
- [x] cancel stream 구현
- [x] log redaction 적용
- [x] log stream은 Product application log와 분리한다.

### TDD/검증

- [x] tail existing file test
- [x] follow appended line test
- [x] cancel tail test
- [x] redaction test
- [x] file not found error test
- [x] max line size truncation test
- [x] max stream duration test

### 완료 기준

- [x] 파일 로그 tail이 streaming된다.
- [x] service shortcut은 systemd/journald 가능 시 동작한다.
- [x] streamed log는 redaction을 거친다.

## 5. 기능 묶음 C. Retention policy

### 작업

- [x] job output retention 기본값 정의
- [x] log stream artifact retention 기본값 정의
- [x] metrics snapshot retention 기본값 정의
- [x] retention cleanup command 구현
- [x] cleanup dry-run 구현
- [x] cleanup audit event 구현

### TDD

- [x] retention cutoff test
- [x] cleanup dry-run test
- [x] audit cleanup event test
- [x] recent artifact not deleted test
- [x] old artifact cleanup test

### 완료 기준

- [x] MVP가 무제한 로그 저장으로 디스크를 채우지 않는다.
- [x] cleanup은 audit에 남는다.
- [x] retention 기본값은 문서화되어 있다.

## 6. 완료 전 체크

- [x] metrics/logs 기능이 observability platform으로 범위 확장되지 않았다.
- [x] Product 로그에 tailed log 원문이 남지 않는다.
- [x] redaction이 application log와 stream output 모두에 적용된다.
