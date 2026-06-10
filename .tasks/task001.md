# Task 001. Production Settings와 Remote URL 경계

상위 계획: `Phase 002`
목표: controller bind address, public external URL, TLS mode, data directory를 명확히 분리하고 remote 운영 CLI UX를 고정한다.
상태: `[ ] 대기` `[ ] 진행 중` `[x] 완료`

## 기능 묶음

- Controller bind/external URL 분리
- Agent/controller URL validation
- 문서와 CLI help 정합성

## 1. 배경

MVP에서는 `http://127.0.0.1:7700` loopback이 중심이었다. 원격 agent를 받으려면 controller가 어디에 bind하는지와 agent가 어떤 public URL로 접근하는지를 분리해야 한다.

예시:

```text
bind address: 0.0.0.0:7700
external URL: https://fleet.example.com
agent init URL: https://fleet.example.com
```

`0.0.0.0`은 listen address일 뿐 agent가 접속할 URL이 아니다.

## 2. 작업

- [x] `ControllerSettings` 또는 동등 settings 구조에 `bind_host`, `bind_port`, `external_url`, `transport_mode`를 명시한다.
- [x] CLI에 `sponzey controller start --external-url https://...`를 추가한다.
- [x] `--host 0.0.0.0`과 `--external-url https://...` 조합을 허용한다.
- [x] `--external-url http://...`는 loopback dev mode에서만 허용한다.
- [x] `agent init --url http://...` non-loopback 거부를 유지한다.
- [x] `agent init --url https://...`는 remote URL로 허용한다.
- [x] URL parser를 ad hoc string split에서 typed parser로 정리한다.
- [x] CLI help에 local dev, SSH tunnel dev, production HTTPS 예시를 분리한다.

## 3. 테스트

- [x] `http://127.0.0.1:7700`은 dev mode에서 허용된다.
- [x] `http://localhost:7700`은 dev mode에서 허용된다.
- [x] `http://192.168.0.10:7700`은 거부된다.
- [x] `http://0.0.0.0:7700`은 거부된다.
- [x] `https://fleet.example.com`은 remote URL로 parse된다.
- [x] `controller start --host 0.0.0.0 --external-url https://fleet.example.com` parse test가 통과한다.
- [x] runtime 중 env var를 읽지 않는지 code search로 확인한다.

## 4. 완료 기준

- [x] remote 운영에서 어떤 URL을 써야 하는지 CLI help만 보고 알 수 있다.
- [x] insecure remote HTTP 우회가 없다.
- [x] 설정값은 bootstrap 시점에만 결정된다.
- [x] README/README.ko.md 예시가 실제 CLI와 일치한다.
