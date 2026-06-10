# Sponzey Fleet

[English](README.md)

Sponzey Fleet는 agent 기반 Fleet 운영 자동화 플랫폼입니다. inbound SSH에 의존하지 않고 서버 자동화, 상태 수집, drift 확인, 명령 실행, 감사 로그, 가벼운 Web Admin UI를 실시간으로 다루기 위한 제품입니다.

핵심 런타임은 Rust입니다. 내부 구현은 역할별 crate로 나뉘지만, 제품은 하나의 `sponzey` 바이너리로 배포합니다.

```text
sponzey controller ...
sponzey agent ...
sponzey enroll-token ...
sponzey run ...
sponzey demo
```

npm 패키지는 Rust 바이너리를 배포하기 위한 wrapper입니다. 런타임 아키텍처가 Node.js라는 의미는 아닙니다.

## 무엇을 하는가

Sponzey Fleet의 현재 MVP는 아래 control loop에 집중합니다.

- 로컬 controller 초기화와 1회용 admin token 생성,
- enrollment token 생성,
- outbound agent 등록,
- agent의 controller identity fingerprint pinning,
- 인증된 WebSocket heartbeat 수신,
- controller-signed task dispatch,
- command output, facts, metrics, drift report, jobs, audit event 수집,
- controller의 `/admin` 경로에서 가벼운 Web Admin UI 서빙.

이 프로젝트는 Ansible 전체 호환 시스템이 아닙니다. 제품 방향은 Ansible 류 자동화, agent 기반 fleet 도구, runner 시스템, 감사 가능한 runbook 플랫폼의 장점만 가져오되 첫 제품을 작고, 테스트 가능하고, 기본적으로 안전하게 유지하는 것입니다.

## 빠른 스토리: Agent 하나를 초기화하고 Web UI 열기

이 스토리는 npm 패키지가 이미 설치되어 있다고 가정합니다. 설치 단계는 여기서 의도적으로 생략합니다.

```bash
npm install -g @sponzey/fleet
sponzey --help
```

소스 저장소에서 로컬 개발로 실행할 때는 Rust CLI를 빌드한 뒤 예시의 `sponzey`를 `./target/debug/sponzey`로 바꾸면 됩니다.

```bash
cargo build -p fleet-cli
./target/debug/sponzey --help
```

### 1. 로컬 Controller 시작

데모용 로컬 data directory를 사용합니다.

```bash
sponzey controller init --data-dir .sponzey
```

`controller init`은 1회용 admin token을 출력합니다. 이 값을 복사해두세요. Web Admin UI를 열거나 보호된 API를 호출할 때 필요합니다.

별도 터미널에서 controller를 시작합니다.

```bash
sponzey controller start \
  --host 127.0.0.1 \
  --port 7700 \
  --data-dir .sponzey \
  --dev-insecure-loopback
```

`--dev-insecure-loopback`은 로컬 `127.0.0.1` 개발 전용입니다. 원격 또는 운영 배포에서 사용하면 안 됩니다.

### 2. Enroll로 Agent 초기화

Sponzey Fleet에서 agent 초기화는 enrollment로 수행합니다. Controller가 짧게 쓰는 enrollment token을 만들고, agent는 같은 data directory에 자기 identity와 pinned controller fingerprint를 저장합니다.

```bash
TOKEN=$(sponzey enroll-token create \
  --data-dir .sponzey \
  --labels role=web,env=dev)

sponzey agent enroll \
  --data-dir .sponzey \
  --url http://127.0.0.1:7700 \
  --token "$TOKEN" \
  --name web-01 \
  --labels role=web,env=dev
```

Agent를 한 번 실행해서 heartbeat, facts, metrics를 보낸 뒤 종료합니다.

```bash
sponzey agent start \
  --data-dir .sponzey \
  --dev-insecure-loopback \
  --once
```

Agent를 foreground loop로 계속 실행하려면 `--once`를 빼면 됩니다.

```bash
sponzey agent start \
  --data-dir .sponzey \
  --dev-insecure-loopback
```

로컬 개발 스크립트도 같은 단일 바이너리를 감싼 것입니다.

```bash
./scripts/run_controller.sh --host 127.0.0.1 --port 7700 --data-dir .sponzey --dev-insecure-loopback
./scripts/run_agent.sh --data-dir .sponzey --dev-insecure-loopback
```

### 3. Web Admin UI에서 Agent 보기

아래 주소를 엽니다.

```text
http://127.0.0.1:7700/admin
```

`sponzey controller init`이 출력한 1회용 admin token을 붙여넣습니다.

MVP Web Admin UI는 의도적으로 가볍게 유지합니다. Controller가 직접 서빙하므로 별도 Node.js web server가 필요하지 않습니다. UI에서는 다음을 확인할 수 있습니다.

- 등록된 agents와 labels,
- 최신 facts와 metrics,
- 최신 drift 결과,
- 명시적 high-risk confirmation을 포함한 command job 생성,
- job output,
- 최근 jobs,
- audit events.

### 4. Agent 삭제하기

현재 MVP에서 가능한 정리는 로컬 agent identity와 설정을 제거하는 방식입니다. 먼저 agent process를 중지한 뒤 로컬 agent directory를 삭제합니다.

```bash
rm -rf .sponzey/agent
```

Controller 쪽 inventory와 audit records는 추적 가능성을 위해 의도적으로 보존합니다. 같은 host를 다시 사용하려면 새 enrollment token을 만들고 `sponzey agent enroll`을 다시 실행합니다.

제품 수준의 삭제 또는 비활성화는 감사 가능한 controller-side 흐름이어야 합니다. 예시는 아래와 같습니다.

```bash
sponzey agents disable <agent-id>
sponzey agents delete <agent-id> --confirm
```

위의 감사 가능한 controller-side 삭제 명령은 제품 방향이며, 현재 MVP command surface는 아닙니다.

## 한 번에 실행하는 로컬 데모

로컬에서 제품 흐름을 빠르게 보고 싶다면 다음을 실행합니다.

```bash
sponzey demo
```

Demo는 로컬 controller를 시작하고, 로컬 agent를 등록하고, sample job을 실행한 뒤 Web Admin URL을 출력합니다.

## Help

사용 가능한 명령은 help로 확인합니다.

```bash
sponzey --help
sponzey controller --help
sponzey controller start --help
sponzey agent --help
sponzey agent enroll --help
sponzey agent start --help
```

로컬 wrapper script는 역할별로 나뉘어 있습니다.

```bash
./scripts/run_controller.sh --help
./scripts/run_agent.sh --help
```

## 안전 모델

Sponzey Fleet는 원격 운영 플랫폼이며, 관리 대상 host에서 root로 실행될 수 있습니다. 따라서 MVP부터 아래 경계를 엄격히 둡니다.

- agent는 controller로 outbound 연결합니다.
- enrollment token은 1회 등록 입력으로 사용합니다.
- agent는 controller fingerprint를 pinning합니다.
- task envelope은 controller가 서명합니다.
- unsigned, expired, replayed, target mismatch task는 agent가 거부합니다.
- high-risk command는 명시적 confirmation이 필요합니다.
- command output은 application log와 분리해서 저장합니다.
- secret은 logs와 audit 성격의 surface에서 redact합니다.
- loopback이 아닌 insecure transport는 거부합니다.

## 개발 검증

변경을 ready로 보기 전 로컬 release gate를 실행합니다.

```bash
./scripts/release_readiness_gate.sh
```

좁은 범위의 유용한 확인 명령은 아래와 같습니다.

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
npm test --workspace web-admin
npm run build --workspace web-admin
./scripts/smoke_mvp.sh
```

## 프로젝트 방향

현재 MVP는 의도적으로 작게 유지합니다. 다음 제품 단계는 아래 방향입니다.

- production TLS deployment,
- 감사 가능한 agent disable/delete,
- 더 견고한 service installation path,
- controller-side retention worker,
- 더 풍부한 runbook execution,
- production key rotation,
- Web Admin UI용 generated API client,
- npm, standalone binary, OS package 기반 packaged release.