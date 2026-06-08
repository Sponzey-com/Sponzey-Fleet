# Task 012. npm 설치와 Service 설치

상위 계획: `M11. npm 설치와 서비스 설치`  
목표: Rust binary를 npm wrapper로 설치하고, demo와 Linux systemd 설치를 제공한다.  
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 진행 메모

- [x] `npm/fleet/package.json` wrapper skeleton 추가
- [x] `npm/fleet/bin/sponzey` Rust binary wrapper 추가
- [x] npm wrapper test 통과
- [x] `scripts/smoke_mvp.sh` local demo script 추가 및 통과
- [x] unsupported platform error 구현
- [x] Cargo workspace version과 npm package version sync test 추가
- [x] local development pack script 작성 및 smoke 통과
- [x] controller/agent install-service dry-run command 구현
- [x] systemd service unit render tests 추가
- [x] `sponzey demo`와 npm wrapper demo smoke 구현 및 통과
- [x] demo 시작 시 `dev-insecure-loopback` Product 로그와 Security audit 기록
- [x] platform optional dependency packages 추가
- [x] systemd 실제 파일 쓰기/start-service command 구현
- [x] `cargo test`, `cargo clippy`, npm wrapper test, local pack smoke 통과
- [x] registry install manual smoke script 추가
- [x] platform package local install smoke script 추가
- [x] systemd reboot manual smoke script 추가
- [x] release readiness gate에 registry publish 이후 install smoke 옵션 연결
- [x] release readiness gate에 reboot 이후 systemd 자동 시작 verify 옵션 연결

## 1. 목적

Sponzey Fleet는 Rust core로 개발하지만, 개발자 진입 장벽을 낮추기 위해 npm 설치 UX를 제공한다. npm은 runtime application이 아니라 Rust 바이너리 배포 wrapper다.

기능 묶음:

- npm binary wrapper
- npx demo
- systemd install

## 2. 선행 조건

- [x] Task 001 완료
- [x] Task 005 완료
- [x] Task 006 완료
- [x] Rust binary가 최소 동작한다.

## 3. 기능 묶음 A. npm binary wrapper

### 작업

- [x] `npm/fleet/package.json` 작성
- [x] platform optional dependency package 구조 설계
- [x] bin shim 작성
- [x] local development pack script 작성
- [x] Cargo version과 npm version sync 방식 정의
- [x] unsupported platform error 구현
- [x] npm package는 Rust binary path만 노출한다.
- [x] npm package가 Node runtime server를 시작하지 않게 한다.

### TDD/검증

- [x] npm package script test
- [x] bin shim points to Rust binary
- [x] unsupported platform error test
- [x] version sync test
- [x] `sponzey --help` after local pack smoke
- [x] `scripts/manual_npm_registry_smoke.sh` 추가
- [x] `scripts/npm_platform_local_install_smoke.sh` 추가
- [x] npm global symlink layout에서 wrapper가 platform binary를 찾는지 검증

### 완료 기준

- [x] local pack 기준 `sponzey --help`가 실행된다.
- [ ] registry publish 이후 `npm install -g @sponzey/fleet` 후 `sponzey --help`가 실행된다.
- [x] npm은 runtime application이 아니라 binary distribution wrapper다.

검증 대기 사유:

- 이 항목은 npm registry publish 이후에만 사실 검증 가능하다.
- publish 전 로컬 tarball/platform package 결합 검증 명령:

```bash
./scripts/npm_platform_local_install_smoke.sh
```

- publish 이후 검증 명령:

```bash
./scripts/manual_npm_registry_smoke.sh
# 또는 전체 release gate와 함께 확인
./scripts/release_readiness_gate.sh --include-registry
```

## 4. 기능 묶음 B. npx demo

### 작업

- [x] `sponzey demo` 구현
- [x] npm wrapper pack 상태에서 demo 실행 smoke 구현
- [x] temp data dir 생성
- [x] local controller 시작
- [x] local agent 등록
- [x] `DevInsecureLoopbackOnly` mode 명시
- [x] loopback URL만 사용하도록 demo scope 제한
- [x] sample command 실행
- [x] browser URL 출력
- [x] port conflict behavior 정의
- [x] temp cleanup behavior 정의

### TDD/검증

- [x] demo command smoke
- [x] temp file cleanup behavior
- [x] port conflict behavior
- [x] non-loopback insecure transport rejected test
- [x] demo Product log marks insecure loopback mode
- [x] demo Security audit marks insecure loopback mode

### 완료 기준

- [x] 5분 안에 local demo가 된다.
- [x] demo mode의 insecure transport는 Product 로그와 audit에 명확히 남는다.
- [x] remote insecure demo는 거부된다.

## 5. 기능 묶음 C. systemd install

### 작업

- [x] `sponzey agent install-service`
- [x] `sponzey agent start-service`
- [x] `sponzey controller install-service`
- [x] service file template 작성
- [x] absolute binary path pinning
- [x] user/group option
- [x] data dir option
- [x] dry-run output
- [x] sudo/root required error 안내

### TDD/검증

- [x] service file render test
- [x] invalid user rejected
- [x] dry-run output test
- [x] absolute binary path test
- [x] missing permission error message test
- [x] `scripts/manual_systemd_reboot_smoke.sh` 추가

### 완료 기준

- [ ] Linux에서 재부팅 후 agent/controller가 자동 시작 가능하다.
- [x] sudo/root가 필요한 작업은 명확히 실패/안내한다.
- [x] service file은 npm global path가 아니라 resolved Rust binary absolute path를 참조한다.

검증 대기 사유:

- 이 항목은 Linux, root 권한, systemd, 실제 reboot가 필요한 manual smoke다.
- Linux 검증 명령:

```bash
sudo ./scripts/manual_systemd_reboot_smoke.sh install
sudo reboot
sudo ./scripts/manual_systemd_reboot_smoke.sh verify

# 또는 release gate와 함께 단계별 확인
sudo ./scripts/release_readiness_gate.sh --include-manual
sudo reboot
sudo ./scripts/release_readiness_gate.sh --verify-manual-reboot
```

## 6. 완료 전 체크

- [x] npm wrapper가 Rust binary distribution 역할만 한다.
- [x] insecure demo는 loopback에서만 동작한다.
- [x] service install은 runtime env mutation을 사용하지 않는다.
- [x] 설치 문서가 있다.