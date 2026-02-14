# WASM MCP Runtime (Decision Draft v0.1)

## Scope
This document scaffolds a future design for running MCP integrations as WebAssembly modules inside Oatty to improve trust boundaries and reduce supply chain risk compared with native process-based MCP servers.

Status: Decision draft v0.1 (not implemented)

## Decision Summary (v0.1)
This draft proposes concrete initial choices so implementation can begin with a stable baseline:

1. Runtime backend:
- Wasmtime + WASI preview 2 component model.

2. Capability policy schema:
- Oatty-owned JSON policy document persisted with module metadata.
- Deny-by-default; explicit allow entries per capability.

3. Signature and trust root model:
- Required content digest pinning for all installs.
- Optional signature verification in phase 1, mandatory in phase 2.
- Ed25519-based publisher keys, with local trust-root store and org allowlist support.

## Problem Statement
Current MCP server integrations commonly execute native binaries or scripts on the user machine, expanding attack surface and trust assumptions.

Target outcome:
- Stronger isolation by default.
- Capability-scoped permissions for MCP integrations.
- Clear install/enable trust UX with auditable decisions.

## Goals
- Define a WASM-based MCP runtime model with explicit capability boundaries.
- Provide deterministic host APIs for MCP request/response operations.
- Minimize blast radius of malicious or compromised integrations.
- Support practical plugin authoring and migration from existing MCP integrations.

## Non-Goals (Phase 1)
- Full compatibility with every native MCP transport feature.
- Arbitrary host process spawning from WASM modules.
- Automatic migration of all existing native MCP plugins.

## Threat Model
### Assets
- Local secrets (tokens, API keys, env vars).
- Filesystem contents.
- User workflows and command catalogs.
- Execution integrity and audit logs.

### Adversaries
- Malicious package/module publishers.
- Compromised upstream dependencies.
- Tampered distribution endpoints.
- Prompt-induced overreach by agents.

### Primary Threats
- Data exfiltration via unconstrained network/file access.
- Privilege escalation via host API abuse.
- Malicious runtime persistence/config mutation.
- Supply chain tampering prior to installation.

## Security Model (Proposed)
### Isolation Boundary
- Execute MCP modules in a WASM runtime sandbox.
- Deny-by-default host capabilities.
- No ambient access to host env/filesystem/network.

### Capability System
Per-module grants (explicit, revocable):
- `network` (allowlist domains/ports, method constraints)
- `filesystem_read` (path-scoped)
- `filesystem_write` (path-scoped)
- `secrets_read` (named secret allowlist)
- `time` (coarse wall clock only)
- `subprocess` (disabled in phase 1)

### Resource Limits
- Memory cap per module.
- CPU/time budget per invocation.
- I/O quotas and maximum response size.
- Concurrency limits.

## Distribution and Trust
### Module Packaging
- Canonical package format (phase 1 proposal): OCI artifact or `.wasm` + sidecar manifest.
- Required metadata: name, version, digest, declared capabilities, entrypoint.

### Provenance and Integrity
- Digest pinning for install and updates.
- Signature verification policy:
  - Phase 1: optional verification with warning on unsigned modules.
  - Phase 2: default deny unsigned modules unless user/org policy overrides.
  - Key model: Ed25519 publisher keys.
  - Trust root: local trusted publisher keyring + optional org-distributed key set.
- Optional organization allowlist policies.

### Install / Enable UX
- Preview permissions before install.
- Show config changes and requested capabilities.
- Explicit user approval required.
- Install does not imply enabled-by-default (proposed).

## Runtime Architecture (Scaffold)
### Components
- WASM runtime host service in `crates/mcp` (new module).
- Capability enforcement middleware.
- Host API bridge for MCP tool lifecycle.
- Audit logger integration.

### Runtime backend decision
- Wasmtime is the initial runtime backend for phase 1.
- Rationale:
  - Mature Rust ecosystem integration.
  - Strong support trajectory for WASI component model.
  - Practical resource-limiting controls and host-call embedding.
- Deferred alternatives:
  - Wasmer
  - WAMR
  - Browser-style isolated runtimes

### Candidate crate/module layout
- `crates/mcp/src/wasm_runtime/`
- `crates/mcp/src/wasm_runtime/host_api.rs`
- `crates/mcp/src/wasm_runtime/capabilities.rs`
- `crates/mcp/src/wasm_runtime/policy.rs`
- `crates/mcp/src/wasm_runtime/loader.rs`

## Host ABI (Initial Draft)
### Required host functions
- `host.log(level, message)`
- `host.http.request(request)` (gated by network policy)
- `host.secret.get(name)` (gated by secret allowlist)
- `host.storage.read(path)` / `host.storage.write(path)` (gated by path policy)
- `host.clock.now()`

### MCP-facing module contract
- Initialize module metadata.
- Enumerate tools/resources/prompts.
- Invoke tool with typed JSON payload.
- Return structured response or typed error.

## Policy Model
### User-level policy
- Per-module capability grants.
- Global defaults by capability class.
- Revocation and quarantine actions.

### Capability policy document (v0.1)
Proposed persisted schema (illustrative):

```json
{
  "module_id": "render-mcp",
  "version": "1.2.3",
  "digest": "sha256:...",
  "trusted": false,
  "capabilities": {
    "network": {
      "allow": true,
      "domains": ["api.render.com"],
      "methods": ["GET", "POST", "PUT", "PATCH", "DELETE"]
    },
    "filesystem_read": { "allow": false, "paths": [] },
    "filesystem_write": { "allow": false, "paths": [] },
    "secrets_read": { "allow": true, "names": ["RENDER_API_KEY"] },
    "time": { "allow": true },
    "subprocess": { "allow": false }
  },
  "limits": {
    "max_memory_mb": 128,
    "max_cpu_ms": 5000,
    "max_io_bytes": 1048576,
    "max_concurrency": 4
  }
}
```

### Org/CI policy (future)
- Trusted publisher allowlist.
- Mandatory signature enforcement.
- Disallow selected capabilities globally.

## Observability and Audit
- Log installation source, digest, and signature status.
- Log permission grants/revocations.
- Log runtime denials (capability violations).
- Correlate tool invocation with module identity/version.

## Revocation and Update Trust Rules
### Revocation model
- Support revocation at three levels:
  - module digest
  - publisher key
  - source registry/repository
- Revoked entities must be denied at load time and flagged in UI.
- Existing enabled modules matching revocation rules must be auto-quarantined.

### Update semantics
- Updates must verify:
  - expected source
  - digest integrity
  - signature status according to policy tier
- Any capability expansion in an update requires re-approval.
- Key rotation requires explicit trust-chain validation (old key signed handoff or org policy override).

## Threat-to-Control Matrix (v0.1)
| Threat | Primary controls | Required tests |
| --- | --- | --- |
| Supply chain tampering | Digest pinning, signature policy, trusted publisher keys | Tampered artifact rejected |
| Malicious host access | Deny-by-default capabilities, host gate middleware | Unauthorized host call denied |
| Secret exfiltration | Named secret allowlist, audit traces | Unlisted secret access denied |
| Data exfil over network | Domain/port/method allowlists | Disallowed network target denied |
| Persistence abuse | Scoped filesystem policy (default deny) | Write attempts outside policy denied |
| Runtime DoS | Memory/CPU/I/O/concurrency limits | Resource cap enforcement tests |

## Security Invariants and Bypass Resistance
- No ambient environment, filesystem, or network access from module context.
- All host interactions must pass a centralized capability gate.
- Direct host-call shortcuts outside policy middleware are prohibited.
- Capability checks are evaluated per invocation and per request target.

## Incident Response and Recovery
- Quarantine mode:
  - disables module execution
  - preserves forensic metadata
  - blocks automatic re-enable until explicit user action
- Emergency controls:
  - global kill switch for WASM modules
  - publisher-key denylist updates
  - digest denylist updates
- Recovery UX:
  - explain quarantine reason
  - show remediation path (update/remove/retrust)

## Verification Strategy
### Adversarial test classes
- Malicious module attempts disallowed host calls.
- Module attempts capability escalation through malformed payloads.
- Module attempts high CPU/memory usage to exceed budgets.
- Module attempts covert exfiltration to unapproved network targets.

### Required validation methods
- Unit tests for policy parsing and gate logic.
- Integration tests for runtime enforcement and audit logging.
- Fuzzing for host ABI input decoding.
- Regression suite for known bypass vectors.

## Compatibility and Migration
### Coexistence model
- Keep native MCP integrations as legacy/high-trust path.
- Introduce WASM MCP as preferred/low-trust path.

### Migration strategy
- Adapter guidance for existing MCP providers.
- Progressive feature parity tracking.
- Explicit compatibility matrix.

## UX Requirements
- Trust-first permission prompts with plain-language explanations.
- Clear labels: installed vs enabled vs trusted.
- One-step disable/uninstall/revoke actions.
- Explain blocked behavior when capability denied.

## Open Questions
- How strict should default network policy be in phase 1 for first-party vs third-party modules?
- Should modules be allowed to persist local state by default?
- What minimum capability set is required for useful MCP plugins?

## Implementation Starter Decisions
These decisions are ready to translate into implementation tasks:

1. Introduce `wasm_runtime` module in `crates/mcp`.
2. Define `WasmModuleManifest` and `WasmCapabilityPolicy` Rust types.
3. Implement digest pin verification as mandatory install gate.
4. Implement capability gate middleware for host calls.
5. Add trust-keyring store under Oatty config directory.

## Acceptance Criteria (Phase 1)
- WASM module can register at least one MCP tool and execute it.
- Capability denials are enforced and user-visible.
- Install and enable flows require explicit approval.
- Runtime logs include module identity/version and policy decisions.
- Native MCP path remains functional and unaffected.

## Phased Rollout
### Phase 0: Design and threat validation
- Finalize threat model and capability taxonomy.
- Decide runtime backend and host ABI v1.

### Phase 1: MVP runtime
- WASM loader, policy engine, limited host ABI.
- Manual install flow with digest pinning.
- Read-only/network-limited tool execution.

### Phase 2: Hardening
- Signature verification and trust policies.
- Resource governance improvements.
- Enhanced audit and admin controls.

### Phase 3: Ecosystem enablement
- Authoring SDK/docs.
- Migration guides and compatibility tooling.
- Optional registry/discovery model.

## Implementation Notes
- Keep this document as forward-looking design; do not mark sections as as-built until implemented.
- Track implementation decisions in linked ADRs (TBD).

## Related specs
- `/Users/justinwilaby/Development/next-gen-cli/specs/PLUGINS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_CATALOG_TOOLS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
