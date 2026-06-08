# Security Checklist

- Enrollment tokens are one-time visible secrets.
- Agent identity is key-pair based.
- Controller identity is key-pair based.
- Agents pin the controller public key after enrollment.
- Task assignments use controller-signed envelopes.
- High-risk commands require explicit confirmation.
- Non-loopback insecure transport is rejected.
- Product logs do not include command output or secret values.

