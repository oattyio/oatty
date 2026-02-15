# WASM MCP Postgres Integration (Decision Draft v0.1)

## Scope

This document defines an initial Postgres-focused WASM MCP integration to validate Oatty's WASM trust model with a
high-value, low-privilege use case.

Status: Decision draft v0.1 (not implemented)

## Motivation

A Postgres integration is a strong first candidate for WASM MCP because it provides immediate operational value while
fitting a constrained capability profile:

- Network-only access to a database endpoint.
- Secret retrieval for DSN/credentials.
- No host filesystem or subprocess access required for core functionality.

Comparable functionality target:

- Similar practical outcomes to native Postgres MCP servers (for example schema introspection and query execution), but
  with stronger isolation and policy controls.

## Goals

- Deliver useful database discovery and querying tools under strict security boundaries.
- Demonstrate capability-gated WASM plugin execution in real workflows.
- Provide deterministic, LLM-friendly JSON outputs with explicit truncation metadata.

## Non-Goals (Phase 1)

- General write access (INSERT/UPDATE/DELETE/DDL).
- Database administration operations (role/db creation, extension install, backup/restore).
- Arbitrary local file import/export features.

## Tool Surface (Phase 1)

### Read-only metadata tools

- `postgres.list_schemas`
- `postgres.list_tables`
- `postgres.describe_table`

### Read-only query tools

- `postgres.run_select`

### Optional convenience tools

- `postgres.list_views`
- `postgres.explain_select`

## Tool Contracts

### `postgres.list_schemas`

Input:

- `database` (optional override)
  Output:
- `schemas: [{ name: string }]`

### `postgres.list_tables`

Input:

- `schema` (optional, default `public`)
  Output:
- `tables: [{ schema: string, name: string, kind: "table" | "partitioned" | "foreign" }]`

### `postgres.describe_table`

Input:

- `schema` (required)
- `table` (required)
  Output:
- `columns: [{ name, data_type, nullable, default, is_primary_key }]`
- `indexes: [{ name, columns, unique }]`

### `postgres.run_select`

Input:

- `query` (required, SELECT-only in phase 1)
- `parameters` (optional positional parameter array)
- `max_rows` (optional, bounded by policy)
- `timeout_ms` (optional, bounded by policy)
  Output:
- `columns: [{ name: string, type: string }]`
- `rows: [object]`
- `row_count: number`
- `truncated: boolean`
- `duration_ms: number`

## Security and Policy Model

### Required capabilities (Phase 1)

- `network`
    - Allowed hosts/ports scoped to user-approved Postgres endpoints.
- `secrets_read`
    - Allowed names for DSN/password secret material.
- `time`
    - For duration/timeout checks.

### Denied capabilities (Phase 1)

- `filesystem_read`
- `filesystem_write`
- `subprocess`

### Policy example (illustrative)

```json
{
  "module_id": "postgres-wasm-mcp",
  "version": "0.1.0",
  "digest": "sha256:...",
  "capabilities": {
    "network": {
      "allow": true,
      "domains": [
        "db.internal.example.com"
      ],
      "ports": [
        5432
      ]
    },
    "secrets_read": {
      "allow": true,
      "names": [
        "POSTGRES_DSN_PROD"
      ]
    },
    "time": {
      "allow": true
    },
    "filesystem_read": {
      "allow": false,
      "paths": []
    },
    "filesystem_write": {
      "allow": false,
      "paths": []
    },
    "subprocess": {
      "allow": false
    }
  },
  "limits": {
    "max_memory_mb": 128,
    "max_cpu_ms": 5000,
    "max_io_bytes": 2097152,
    "max_concurrency": 4,
    "default_query_timeout_ms": 3000,
    "max_query_timeout_ms": 10000,
    "default_max_rows": 100,
    "max_rows": 1000,
    "max_query_length": 20000
  }
}
```

## Query Safety Gates (Phase 1)

- Enforce read-only transaction mode where driver/runtime supports it.
- Permit only SELECT/CTE-read queries.
- Deny statements containing write/admin operations (`INSERT`, `UPDATE`, `DELETE`, `MERGE`, `ALTER`, `DROP`, `TRUNCATE`,
  `CREATE`, `GRANT`, `REVOKE`, `COPY`, `CALL`, `DO`).
- Reject multi-statement payloads.
- Enforce timeout and row caps.

Note: Keyword filtering is a first-line guard only. Primary enforcement should be protocol/session-level read-only
controls plus statement class checks where possible.

## Secrets and Connection Handling

- DSN is referenced by secret name and resolved via Oatty secret store.
- Never log raw DSN, password, or full query literals containing potential sensitive values.
- Prefer connection pooling scoped to module instance with bounded pool size.

## Update and Revocation Rules (Postgres Module)

- Any update that changes:
    - requested network targets
    - requested secret names
    - capability set
      requires explicit re-approval.
- Revoked digest or publisher key results in automatic module quarantine.
- Quarantined Postgres modules cannot execute tools until user resolves trust state.

## Observability and Audit

Log per invocation:

- `module_id`, `module_version`, `tool_name`
- query fingerprint (normalized hash), not raw sensitive text by default
- `duration_ms`, `row_count`, `truncated`, timeout/denial reason
- capability denials and policy decision context

### Incident response logging

- Emit explicit events for:
    - digest/signature verification failures
    - capability policy denials
    - quarantine transitions
    - emergency kill-switch activation

## UX Requirements

- Install prompt must clearly show:
    - requested network targets
    - secret names requested
    - denied-by-default capabilities
- Error messages should be actionable:
    - "query rejected: write operations are disabled in phase 1"
    - "query timeout exceeded policy max"
    - "row limit exceeded; response truncated"

## Compatibility Notes

- Intended to coexist with native MCP Postgres servers.
- WASM variant should be labeled preferred low-trust path when policy requirements are satisfied.

## Acceptance Criteria (Phase 1)

- Module installs with digest pin and explicit permission approval.
- `list_schemas`, `list_tables`, `describe_table`, and `run_select` execute successfully against allowed endpoints.
- Write/admin SQL attempts are blocked with structured errors.
- Timeout and row cap enforcement verified by tests.
- Audit logs include module identity and query fingerprint metadata.
- Revoked digest/key causes module quarantine and execution denial.

## Phase 2 Expansion (Candidate)

- Optional elevated write mode with explicit user grant and session prompts.
- Parameterized safe templates for common mutations.
- Optional RLS/tenant policy integrations.

## Implementation Kickoff Checklist

### Slice 1: Types and policy contracts

- Add `WasmModuleManifest` and `WasmCapabilityPolicy` Rust types in `crates/mcp`.
- Add serde validation for:
    - required digest
    - bounded limits
    - deny-by-default capability defaults
- Tests:
    - valid/invalid policy parsing
    - limit boundary validation

### Slice 2: Runtime loader + capability gate stubs

- Create `crates/mcp/src/wasm_runtime/` module skeleton:
    - `loader.rs`
    - `policy.rs`
    - `capabilities.rs`
    - `host_api.rs`
- Implement capability gate checks before host calls.
- Tests:
    - denied capability returns structured policy error
    - allowed capability path reaches stub host API

### Slice 3: Postgres metadata MVP tool

- Implement `postgres.list_schemas` on WASM runtime path.
- Enforce required capabilities:
    - `network`
    - `secrets_read`
    - `time`
- Add audit emission for invocation metadata.
- Tests:
    - happy path with mocked DB response
    - denied network capability
    - missing secret permission

### Slice 4: `postgres.run_select` with phase-1 safety gates

- Implement SELECT-only execution with:
    - read-only transaction/session mode
    - multi-statement rejection
    - timeout/row caps
- Return deterministic output envelope (`columns`, `rows`, `row_count`, `truncated`, `duration_ms`).
- Tests:
    - valid select succeeds
    - write/admin query rejected
    - timeout and truncation behavior

### Slice 5: Install/enable trust UX wiring

- Add install preview output for:
    - digest
    - requested capabilities
    - secret names and network targets
- Require explicit user approval before enablement.
- Tests:
    - install preview payload shape
    - enable blocked without approval

### Exit Criteria for kickoff phase

- First metadata tool and `run_select` work end-to-end in controlled environment.
- Capability denials and safety gate errors are user-actionable.
- Audit records capture module identity, policy decisions, and query fingerprint metadata.
- No regressions in native MCP plugin flow.

## Open Questions

- Which Rust Postgres client stack best fits WASM component constraints?
- How should query fingerprinting and redaction be standardized across integrations?
- Should write-mode be a separate module identity or a capability toggle?

## Threat-to-Control Mapping (Postgres v0.1)

| Threat                         | Control                                 | Test expectation                                |
|--------------------------------|-----------------------------------------|-------------------------------------------------|
| Exfiltration to arbitrary host | Network allowlist policy                | Query to non-allowlisted host denied            |
| Secret overreach               | `secrets_read` name allowlist           | Access to unapproved secret denied              |
| Write/query abuse              | Read-only gate + statement restrictions | DML/DDL statements rejected                     |
| Data overexposure              | Row caps and truncation metadata        | Oversized result set truncated with flag        |
| Resource exhaustion            | Timeout and CPU/memory caps             | Long-running query terminated deterministically |

## Adversarial Verification Addendum

- Attempt SQL payloads with obfuscation/mixed case to bypass write-operation filters.
- Attempt multi-statement chains with comments/whitespace tricks.
- Attempt denial-of-service via large result scans and cartesian joins.
- Validate that raw sensitive literals are redacted in logs while preserving forensic usefulness.

## Source Alignment Targets (Future)

- `crates/mcp/src/wasm_runtime/*`
- `crates/mcp/src/server/*`

## Related specs

- `WASM_MCP_RUNTIME.md`
- `PLUGINS.md`
- `LOGGING.md`
- `MCP_WORKFLOWS.md`
