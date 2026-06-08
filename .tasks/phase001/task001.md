# Task 001. Rust 프로젝트 골격

상위 계획: `M0. 프로젝트 골격`  
목표: Rust workspace, CLI skeleton, settings/logging bootstrap을 만든다.  
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 1. 목적

MVP의 첫 작업은 agent enrollment가 아니다. 먼저 Rust workspace와 실행 진입점, 설정/로그 규칙을 잡아야 이후 Controller, Agent, CLI, protocol, storage가 Clean Architecture 경계를 지키며 올라갈 수 있다.

이 task는 다음 3개 기능 묶음만 다룬다.

- Rust workspace와 crate skeleton
- `sponzey` CLI command skeleton
- typed `Settings`, `LogProfile`, `TransportSecurityMode` bootstrap

## 2. 선행 조건

- [x] `AGENTS.md`의 Layered Architecture, Clean Architecture, Tidy First, TDD 규칙을 읽었다.
- [x] `.tasks/plan.md`의 `M0`과 `MVP 보안 바닥선`을 읽었다.
- [x] Rust toolchain이 설치되어 있다.
- [x] 이 task에서는 비즈니스 기능 구현을 시작하지 않는다는 범위를 확인했다.

## 3. 범위

포함:

- [x] root `Cargo.toml` workspace 생성
- [x] MVP crate skeleton 생성
- [x] CLI command tree 생성
- [x] settings/logging bootstrap 생성
- [x] architecture dependency policy 초안 작성

제외:

- [x] enrollment token 실제 발급
- [x] WebSocket transport
- [x] SQLite repository 구현
- [x] command runner 구현
- [x] Web Admin UI 구현

## 4. 기능 묶음 A. Rust workspace

### 작업

- [x] root `Cargo.toml`에 workspace를 정의한다.
- [x] workspace resolver를 명시한다.
- [x] Rust edition을 통일한다.
- [x] 공통 dependency 후보를 workspace dependency로 정리한다.
- [x] 다음 crate를 만든다.

  - [x] `crates/fleet-domain`
  - [x] `crates/fleet-application`
  - [x] `crates/fleet-protocol`
  - [x] `crates/fleet-store`
  - [x] `crates/fleet-runner`
  - [x] `crates/fleet-controller`
  - [x] `crates/fleet-agent`
  - [x] `crates/fleet-cli`
- [x] 각 crate에 최소 `lib.rs` 또는 `main.rs`를 둔다.
- [x] `fleet-domain`은 framework, async runtime, DB, HTTP dependency를 갖지 않게 한다.
- [x] `docs/architecture.md` 또는 동등 문서에 dependency policy를 적는다.
- [x] architecture fitness check 후보 script를 설계한다.

### 테스트/검증

- [x] `cargo check --workspace`
- [x] `cargo test --workspace`
- [x] `cargo tree` 또는 동등 명령으로 `fleet-domain` 의존성을 확인한다.
- [x] `fleet-domain`에 `tokio`, `sqlx`, `axum`, `reqwest` 같은 outer dependency가 없음을 확인한다.

### 완료 기준

- [x] 모든 crate가 workspace에 포함된다.
- [x] workspace가 빈 껍데기라도 compile 가능하다.
- [x] architecture dependency policy가 문서화되어 있다.

## 5. 기능 묶음 B. CLI command skeleton

### 작업

- [x] `fleet-cli`에 `sponzey` binary를 만든다.
- [x] `clap` 기반 command tree를 만든다.
- [x] 다음 명령을 skeleton으로 추가한다.

  - [x] `sponzey controller init`
  - [x] `sponzey controller start`
  - [x] `sponzey agent enroll`
  - [x] `sponzey agent start`
  - [x] `sponzey agents list`
  - [x] `sponzey enroll-token create`
  - [x] `sponzey run`
  - [x] `sponzey facts`
  - [x] `sponzey metrics`
  - [x] `sponzey logs`
  - [x] `sponzey drift check`
- [x] 구현 전 명령은 명확한 `NotImplemented` 오류를 반환한다.
- [x] panic 없이 exit code와 user-facing message를 반환한다.
- [x] CLI parser가 env var를 읽지 않게 한다.

### 테스트/검증

- [x] CLI parser unit test
- [x] invalid command test
- [x] `sponzey --help` snapshot 또는 smoke test
- [x] not implemented command exit code test

### 완료 기준

- [x] `sponzey --help`가 MVP 명령을 보여준다.
- [x] 구현되지 않은 명령이 panic하지 않는다.
- [x] CLI skeleton이 settings/runtime 변경을 수행하지 않는다.

## 6. 기능 묶음 C. Settings와 logging bootstrap

### 작업

- [x] `Settings` typed struct를 정의한다.
- [x] `LogProfile` enum을 정의한다.

  - [x] `Product`
  - [x] `FieldDebug`
  - [x] `Development`
- [x] `TransportSecurityMode` enum을 정의한다.

  - [x] `TlsRequired`
  - [x] `DevInsecureLoopbackOnly`
- [x] `DevInsecureLoopbackOnly`는 loopback address에서만 유효하게 validation한다.
- [x] settings는 CLI args와 최초 config file에서만 생성한다.
- [x] runtime settings mutation API를 만들지 않는다.
- [x] `tracing` 기반 logging bootstrap을 만든다.
- [x] Product 로그를 기본값으로 둔다.
- [x] secret redaction helper skeleton을 만든다.

### 테스트/검증

- [x] settings validation test
- [x] invalid bind address test
- [x] invalid log profile test
- [x] insecure remote URL rejected test
- [x] insecure loopback accepted test
- [x] redaction helper test
- [x] production code에서 `std::env::set_var`가 없는지 검색한다.

### 완료 기준

- [x] controller/agent/cli는 bootstrap에서 만든 `Settings`를 명시적으로 받는다.
- [x] request handler나 application service가 env var를 읽지 않는다.
- [x] non-loopback URL은 TLS 없이는 validation에서 거부된다.

## 7. 산출물

- [x] root `Cargo.toml`
- [x] MVP crate skeleton
- [x] `sponzey` CLI skeleton
- [x] settings/logging bootstrap 코드
- [x] architecture dependency policy 문서
- [x] 최소 테스트

## 8. 완료 전 체크

- [x] Tidy 작업과 behavior 작업을 섞지 않았다.
- [x] domain crate가 outer layer dependency를 갖지 않는다.
- [x] 설정은 bootstrap 시점에만 만들어진다.
- [x] Product 로그 기본값이 적용된다.
- [x] insecure mode는 loopback 전용이다.