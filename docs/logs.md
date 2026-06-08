# Sponzey Fleet MVP Logs

MVP log handling is a field diagnostic surface. It is not a log aggregation or archival product.

## File Tail

```bash
sponzey logs --file /var/log/syslog
sponzey logs web-01 --file /var/log/syslog --follow --max-duration-seconds 30
```

Current behavior:

- reads the target file from the local process filesystem,
- emits the last 50 lines first,
- redacts secret-like values before display,
- truncates oversized lines,
- with `--follow`, polls the same file for appended lines,
- with `--max-duration-seconds`, exits the follow loop after the requested duration.

The optional `target` argument is accepted for operator context, but MVP file tail does not yet open a remote file through an agent task.

## Journald Shortcut Skeleton

When no `--file` is provided and the target looks like a safe systemd unit name, the CLI renders the intended journald command:

```bash
sponzey logs nginx.service
```

This is a skeleton for the later systemd/journald adapter. It validates the service name and does not shell-execute untrusted input.

## Boundaries

- Product application logs do not include tailed log lines.
- Log stream output is redacted independently from application logging.
- Log tail artifacts are not persisted separately in MVP.
- Remote agent log streaming remains a later signed task/streaming protocol feature.
