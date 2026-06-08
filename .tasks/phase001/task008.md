# Task 008. Task Primitives와 Runbook

상위 계획: `M7. Task Primitives와 Runbook`  
목표: YAML runbook parser와 package/service/file.copy primitive를 구현한다.  
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 진행 메모

- [x] sample policy 파일 추가
- [x] CLI `sponzey drift check --policy`에서 policy file read skeleton 구현
- [x] 제한된 MVP runbook YAML parser 구현
- [x] `sponzey apply <file>` validation-only command 구현
- [x] runbook JSON schema export 함수와 schema test 추가
- [x] package/service primitive command builder 구현
- [x] file.copy runner primitive 실제 실행 구현
- [x] file.copy permission error mapping과 atomic fallback test 구현
- [x] runbook parser 결과를 package/service/file.copy primitive execution plan으로 낮추는 builder 구현
- [x] runner-level runbook execution plan 실행기 구현
- [x] high-risk runbook command는 confirmation 없이는 실행 전 차단
- [x] package/service primitive 실제 실행과 runbook 실행 연결 구현
- [x] Linux nginx runbook manual smoke test와 실행 스크립트 추가
- [x] `/api/jobs/runbook` signed runbook dispatch API 구현
- [x] SQLite runbook job payload 저장과 pending assignment 조회 구현
- [x] agent-side signed runbook task 실행 경로 구현
- [x] manual Linux nginx smoke를 controller API + signed WebSocket task + agent 실행 흐름으로 전환

## 1. 목적

MVP는 Ansible full compatibility를 목표로 하지 않는다. 대신 자주 쓰는 운영 primitive를 작고 검증 가능한 YAML runbook으로 실행한다.

기능 묶음:

- Runbook parser
- package/service primitive
- file.copy primitive

## 2. 선행 조건

- [x] Task 006 완료
- [x] command dispatch와 signed task envelope가 동작한다.
- [x] Task 007의 selector가 job dispatch에 연결되어 있다.

## 3. 기능 묶음 A. Runbook parser

### 작업

- [x] YAML schema 정의
- [x] `apiVersion` 필수화
- [x] `kind` 필수화
- [x] `metadata.name` 필수화
- [x] `spec.targets` 필수화
- [x] `spec.tasks` 필수화
- [x] JSON schema export 구현
- [x] validation error formatting 구현
- [x] unsupported field rejection 구현
- [x] Ansible compatibility를 암시하지 않는 문구 작성

### TDD

- [x] valid nginx runbook parse
- [x] missing targets rejected
- [x] missing tasks rejected
- [x] unsupported task rejected
- [x] invalid YAML error
- [x] schema fixture test
- [x] unknown top-level field rejected

### 완료 기준

- [x] `sponzey apply playbook.yml`가 runbook을 읽고 validation한다.
- [x] validation error가 사용자가 고칠 수 있는 형태로 나온다.
- [x] Ansible full compatibility를 암시하지 않는다.

## 4. 기능 묶음 B. package/service primitive

### 작업

- [x] Linux package manager detection 구현

  - [x] apt
  - [x] dnf/yum
  - [x] apk는 후순위 가능
- [x] package present check command builder 구현
- [x] package install command builder 구현
- [x] systemd service status command builder 구현
- [x] systemd service start/restart command builder 구현
- [x] systemd service enable command builder 구현
- [x] changed true/false 반환 구현
- [x] dry-run 준비 구조 정의
- [x] service restart는 high-risk confirmation/approval boundary를 통과해야 한다.

### TDD/검증

- [x] command builder unit test
- [x] package already installed fixture
- [x] package missing fixture
- [x] service status parser test
- [x] systemd unavailable behavior test
- [x] dangerous restart approval hook test
- [x] changed false when already desired test
- [x] `manual_linux_nginx_runbook_executes` ignored integration test 추가
- [x] `scripts/manual_linux_nginx_runbook_smoke.sh` 추가
- [x] runbook job creation application test
- [x] runbook protocol roundtrip test
- [x] controller runbook REST API signed assignment test
- [x] SQLite pending runbook assignment test
- [x] Web Admin shared API client surface test

### 완료 기준

- [ ] nginx install/start runbook이 Linux에서 동작한다.
- [x] primitive command builder output은 구조화된다.
- [x] package/service primitive는 idempotent 결과를 반환한다.

검증 대기 사유:

- 이 항목은 Linux, root 권한, systemd, package manager가 필요한 destructive manual smoke다.
- macOS 개발 환경에서는 완료로 체크하지 않는다.
- 스크립트는 controller init, agent enrollment, `/api/jobs/runbook` 생성, signed WebSocket dispatch, agent execution까지 포함한다.
- Linux 검증 명령:

```bash
sudo ./scripts/manual_linux_nginx_runbook_smoke.sh
```

## 5. 기능 묶음 C. file.copy primitive

### 작업

- [x] inline content MVP strategy 정의
- [x] destination path validation 구현
- [x] mode MVP 범위 정의
- [x] owner/group MVP 범위 정의
- [x] checksum before/after 구현
- [x] atomic write 구현 가능한 경우 적용
- [x] path safety guard 구현
- [x] destructive overwrite audit metadata를 남긴다.

### TDD

- [x] copy creates file
- [x] unchanged file returns changed=false
- [x] checksum mismatch handled
- [x] unsafe path rejected
- [x] permission error mapped
- [x] atomic write fallback behavior test

### 완료 기준

- [x] runbook에서 file copy가 동작한다.
- [x] file write는 audit 가능한 job step으로 남는다.
- [x] unsafe path는 실행 전 거부된다.

## 6. 완료 전 체크

- [x] YAML parser가 domain/application rule을 우회하지 않는다.
- [x] primitive 실행은 signed task envelope 내부에서만 수행된다.
- [x] high-risk primitive는 confirmation/approval boundary를 가진다.
- [x] Ansible full compatibility 범위로 확장하지 않았다.
