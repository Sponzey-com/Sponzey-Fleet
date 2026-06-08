# Logging Guide

Sponzey Fleet uses three log profiles:

- `Product`: minimal operational logs, default.
- `FieldDebug`: field diagnosis with secret redaction.
- `Development`: local development and tests.

Command output belongs in job output storage, not product application logs.

