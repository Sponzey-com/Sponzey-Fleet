# Task 009. Packaging/Release Hardening

상위 계획: `Phase 002`
목표: 원격 운영 beta를 배포할 수 있도록 npm, standalone binary, Linux compatibility, smoke gate를 강화한다.
상태: `[ ] 대기` `[x] 진행 중` `[ ] 완료`

## 기능 묶음

- Release artifact compatibility
- npm/standalone distribution
- Beta release gate

## 1. 작업

- [x] Linux glibc baseline 정책을 문서화한다.
- [ ] musl static build 가능성을 spike하고 선택 여부를 결정한다.
- [x] npm package platform matrix에 compatibility check를 추가한다.
- [x] release workflow에서 `strings bin/sponzey | rg GLIBC_` 검사를 자동화한다.
- [x] standalone binary artifact를 GitHub Release에 업로드한다.
- [x] checksum 파일을 생성한다.
- [x] release notes template에 known limitations와 upgrade notes를 추가한다.
- [x] remote HTTPS smoke를 release gate에 포함한다.

## 2. 테스트/검증

- [x] npm wrapper install smoke
- [x] current platform package smoke
- [x] Linux x64 glibc requirement check
- [x] Linux arm64 glibc requirement check
- [x] HTTPS remote smoke
- [x] `sponzey agent init --help` registry smoke
- [ ] release workflow dry-run

## 3. 완료 기준

- [x] 사용자가 npm으로 설치했을 때 최신 CLI UX를 바로 쓸 수 있다.
- [x] Linux 호환성 문제가 release 전에 발견된다.
- [x] standalone binary를 npm 없이도 받을 수 있다.
- [x] Phase 002 beta release가 재현 가능한 gate를 가진다.