# Security Checklist

- Enrollment tokens are one-time visible secrets.
- Agent identity is key-pair based.
- Controller identity is key-pair based.
- Agents pin the controller public key after enrollment.
- Task assignments use controller-signed envelopes.
- High-risk commands require explicit confirmation.
- HTTP transport is test-only, emits warnings, and writes Security audit when configured as the controller external URL.
- Product, customer, production, shared, and long-running environments use HTTPS.
- Product logs do not include command output or secret values.
