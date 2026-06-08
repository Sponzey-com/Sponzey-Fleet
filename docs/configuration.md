# Configuration Guide

Sponzey Fleet accepts external configuration only during process bootstrap.

Rules:

- Do not mutate process environment at runtime.
- Do not add runtime configuration mutation endpoints.
- Pass settings explicitly through typed `Settings`.
- `DevInsecureLoopbackOnly` is allowed only for loopback demo mode.

