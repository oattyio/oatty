# Security Policy

Report vulnerabilities privately to the maintainers.

Do not include secrets in issues or logs.

Tokens and sensitive values must be redacted in output and telemetry by default.

## Configuration safety notes

- Catalog authentication is configured per catalog/library entry; avoid global token environment variables in docs/scripts.
- MCP secrets should use `${secret:NAME}` interpolation and OS keychain storage.
- Runtime logs can be persisted for diagnostics; avoid pasting raw logs that may contain sensitive metadata.
