Title: MCP Basic Authorization (TUI-first) via ultrafast-mcp

Overview
- Goal: Implement MCP Basic Authorization aligned with MCP Basic spec (2025-03-26) with a TUI-first UX, migrating client/transport from rmcp to ultrafast-mcp behind a feature flag.
- Scope: Stdio and HTTP/SSE transports, keyring-backed secret storage, TUI auth UI (status, modal, actions). CLI UX deferred.
- Safety: Redact all secrets; one interactive retry on auth failure; preserve rmcp fallback until ultrafast path is stable.

Key References
- Spec: MCP Basic Authorization (2025-03-26)
- Crate: ultrafast-mcp 202506018.1.0 (modules: AuthTypes, protocol, streamable_http, middleware; traits: ClientElicitationHandler, Transport/HealthCheck)

Architecture Changes
- Replace rmcp with ultrafast-mcp in `crates/mcp`; no fallback needed.
- Swap internals of `crates/mcp/src/client/{stdio,http}.rs` to use ultrafast transports while preserving our public traits `McpTransport`/`McpConnection`.
- Implement Basic auth at two layers:
  - HTTP/SSE: TransportMiddleware injecting `Authorization: Basic <base64(user:pass)>`.
  - Stdio: ClientElicitationHandler to respond to auth negotiation/authorize with Basic credentials.
- Secrets: Resolve from keyring/env/config; store via keyring and support `${secret:NAME}` in config interpolation.

Data/Types
- New (mcp crate):
  - enum AuthStatus { Unknown, Authorized, Required, Failed(String) }
  - struct PluginAuthConfig { scheme: Basic, source: Keyring|Env|Inline; interactive: bool }
  - SecretStore trait (backed by keyring) if not already present elsewhere.
- Use ultrafast-mcp types:
  - TransportConfig (stdio/http)
  - ClientElicitationHandler (for stdio auth challenges)
  - Middleware trait (for HTTP auth header injection)
  - AuthResult/McpAuthError (map to our errors)

Transport & Client Wiring
- Stdio (ultrafast):
  - Build via `create_transport(TransportConfig::Stdio { command, args, env, cwd })`.
  - Provide a ClientElicitationHandler that supplies Basic credentials when server requests authorization; on rejection, optionally prompt (interactive), then retry once.
  - After successful authorization, mark plugin AuthStatus=Authorized.
- HTTP/SSE (ultrafast streamable):
  - Build via `create_streamable_http_client_with_middleware`.
  - Install BasicAuth middleware: constructs and injects `Authorization` header from resolved credentials.
  - Health checks use the same client with a lightweight GET to `/health` or `/ping`.

TUI UX
- Plugins table: add an “Auth” column with status chips: Ok, Required, Failed, Unknown.
- Context menu actions: Set Credentials, Clear Credentials, Test Auth.
- Auth modal: capture token or username/password, masked inputs, “save to keyring” toggle. Emits Effects to manager.
- Effects/Commands:
  - Effect::SetPluginCredentials(name, creds)
  - Effect::ClearPluginCredentials(name)
  - Effect::TestPluginAuth(name)
  - Manager maps to: save/delete via keyring; trigger authorize/test via client.

Configuration
- Extend `~/.config/heroku/mcp.json` per server (non-breaking):
  - `auth`: { "scheme": "basic", "secret": "MCP_<NAME>_TOKEN" } or { "username_env": "...", "password_env": "..." }
  - `interactive`: true|false (default true)
- HTTP servers may still specify headers; Basic middleware supersedes manual header when configured.

Error Handling & Redaction
- Recognize auth failures via ultrafast `McpAuthError` or HTTP 401/403; map to `AuthStatus::Failed(reason)` (redacted).
- Redact secrets in logs using existing `heroku_util` helpers; never log header values; UI shows token tail only.

Testing Plan
- Unit
  - Basic header construction (base64 of `username:password`; username-only or token forms as supported).
  - Elicitation handler retry logic and state transitions.
  - Keyring store get/set/delete with mock.
- Integration
  - Stdio mock: unauthorized → interactive retry → authorized (handler tested end-to-end).
  - HTTP middleware: verify header injection on request.
- CI: `cargo fmt`, `clippy -D warnings`, `test --workspace` must pass.

Step-by-Step Tasks
1) Dependencies & Feature Flags
   - Add `ultrafast-mcp = "202506018.1.0"` to workspace deps.
   - In `crates/mcp/Cargo.toml`, add feature `mcp-ultrafast` and cfg-gate new code.
2) HTTP Transport Migration (behind feature)
   - Implement HTTP client using `create_streamable_http_client_with_middleware`.
   - Add BasicAuth middleware for header injection.
   - Keep existing rmcp-backed HTTP code as fallback (for now).
3) Stdio Transport Migration (behind feature)
   - Create stdio transport via `create_transport(TransportConfig::Stdio{...})`.
   - Implement `ClientElicitationHandler` for Basic:
     - Resolve credentials; reply to challenge.
     - On rejection: if interactive, fetch new creds via TUI path; retry once.
4) Manager Integration
   - Thread optional PluginAuthConfig per server; resolve credentials via keyring/env.
   - Add authorize/test flows; update `AuthStatus` on success/failure.
5) TUI
   - Add AuthStatus to app state; render “Auth” column in plugins table.
   - Add context menu actions for set/clear/test; modal for credentials.
6) Redaction
   - Ensure all logs and UI outputs redact secret material.
7) Tests & Docs
   - Add unit/integration tests described above.
   - Update specs/PLUGINS.md with auth UX notes; document config fields.

Acceptance Criteria
- HTTP and stdio transports authorize using Basic per MCP spec when configured.
- TUI displays accurate auth status and supports set/clear/test.
- Secrets never leak; keyring storage works; one interactive retry on failure.
- rmcp fully removed; transports powered by ultrafast-mcp.

Open Questions
- Exact Basic helper APIs in `ultrafast-mcp-auth` (if present). If absent, proceed with middleware + protocol authorize request via elicitation handler.
- HTTP/SSE response correlation under streamable_http for our provider flows—verify tool call path.
