MCP Plugin Management Specification (Minimal)
This document specifies a Terminal User Interface (TUI) for managing Model Context Protocol (MCP) plugins within the Heroku CLI, aligned with foundational MCP client capabilities (e.g., Cursor/Claude compatibility). The TUI provides a fast, intuitive, and portable interface for discovering, installing, configuring, and monitoring plugins, using a minimal mcp.json configuration stored at ~/.config/heroku/mcp.json. Built with Ratatui (Rust) and the Nord theme, it prioritizes developer-friendliness, keyboard-driven navigation, and minimal latency (<50ms render time). The design supports stdio and http/sse transports and assumes permissions are managed externally via role-based access control (RBAC) or attribute-based access control (ABAC) systems, which define fine-grained authorization for MCP-connected tools or resources.

Implementation status (MVP)
- Config: Uses ~/.config/heroku/mcp.json with camelCase keys (mcpServers, baseUrl). MCP_CONFIG_PATH override supported; ${env:} and ${secret:} interpolation implemented.
- Transports: stdio implemented via rmcp child transport; http/sse transport health checks implemented, protocol connect is a stub (future work).
- Lifecycle/Health: Lifecycle manager with backoff/timeouts; periodic health checks using each client’s real health_check().
- Logging: Ring buffer + audit logs; redaction in display; export supports redacted or unredacted output.
- Registry/Tags: Registry wired; tags parsed from config and propagated to engine/registry.
- Validation: Server names and env keys validated; http baseUrl requires http/https.

1. Goals

Portability: Use a minimal mcp.json schema compatible with all MCP clients, avoiding Heroku-specific extensions.
Speed: Low-latency TUI interactions using core Ratatui widgets.
Intuitive UX: Keyboard-first navigation, clear feedback (toasts, logs), and discoverable actions (hint bar, search).
Safety: Out-of-process plugins (stdio) and remote execution (http/sse) to prevent crashes.
Simplicity: Focus on core MCP features (plugin discovery, invocation, configuration) for fast adoption.
Extensibility: Provide hooks for future enhancements without overcomplicating the MVP.


2. Configuration (mcp.json)
2.1 File Location

Primary: ~/.config/heroku/mcp.json (read/write).
No Fallback: No support for ~/.cursor/mcp.json to simplify configuration management.

2.2 Schema
The configuration uses the minimal MCP-standard mcpServers object, supporting only essential fields for maximum compatibility.
{
  "mcpServers": {
    "server-name": {
      "command": "node",                       // for stdio
      "args": ["-e", "require('@mcp/server').start()"],
      "env": {
        "FOO": "bar",
        "HEROKU_API_TOKEN": "${env:HEROKU_API_TOKEN}" // process env interpolation
      },
      "cwd": "/path/optional",
      "disabled": false
    },
    "remote-example": {
      "baseUrl": "https://mcp.example.com",
      "headers": {
        "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}" // keychain interpolation
      },
      "disabled": false
    }
  }
}

2.3 Notes

Fields: command, args, env, cwd for stdio; baseUrl, headers for http/sse; disabled for enabling/disabling plugins.
Interpolation:
${env:NAME}: Reads process environment at runtime, not persisted.
${secret:NAME}: Resolves via OS keychain (e.g., keyring-rs); never persisted.


Permissions: Handled externally via RBAC/ABAC systems, not defined in mcp.json or enforced by the TUI.

2.4 Validation

Name: ^[a-z0-9._-]+$.
Fields:
stdio: Requires command; optional args, env, cwd.
http/sse: Requires baseUrl; optional headers.


Env Keys: ^[A-Z_][A-Z0-9_]*$.


3. TUI Architecture
3.1 Global Shell

Regions: Title bar, hint bar (footer with keybindings), status line (top-right, e.g., "Plugins: 2").
Colors (Nord):
Background: #2E3440, Panels: #3B4252, Border: #434C5E.
Text: Primary #D8DEE9, Secondary #E5E9F0, Muted #4C566A.
Accents: Teal #88C0D0, Blue #81A1C1 (selected), Status: Green #A3BE8C (ok), Yellow #EBCB8B (warn), Red #BF616A (err).


Keybindings (Global):
Tab/Shift-Tab: Cycle focus.
F1: Help screen.
/: Quick search.
q: Back/close.
Esc: Cancel.
:: Command palette.



3.2 Screens and Widgets

Plugins List

Purpose: List installed plugins, show status, jump to details.
Layout (ASCII):┌ Plugins — MCP ────────────────────────────────────────┐
│ / search ▎github ...                                                 │
├──────────────────────────────────────────────────────────────────────┤
│ Name       Status    Command/BaseUrl                  Tags           │
├──────────────────────────────────────────────────────────────────────┤
│ github     ✓ Running npx -y @mcp/server-gh            code,gh        │
│ pg-remote  ✗ Stopped https://mcp.example.com          pg,db          │
└──────────────────────────────────────────────────────────────────────┘
Hints: Enter details • a add • d disable/enable • r restart • L logs


Behaviors: Arrow/j/k navigation, zebra striping, search (/) across name/tags, status icons (✓ ok, ✗ stopped, ! warn).
Widget: ratatui::widgets::Table.


Plugin Details

Purpose: Show configuration, health, environment, logs, and actions.
Layout:┌ github — Details ──────────────────────────────────────────┐
│ Overview    Health    Env    Logs                                 │
├───────────────────────────────────────────────────────────────────┤
│ Command: npx -y @mcp/server-gh       Timeout: 20s                 │
│ Tags: code, gh                       Last start: 12:41:03         │
│                                      Handshake: 180ms             │
│                                                    [R] Restart    │
├───────────────────────────────────────────────────────────────────┤
│ Env: GITHUB_TOKEN=••••••     USER=heroku-mcp/2.1   [E] Edit       │
├───────────────────────────────────────────────────────────────────┤
│ Logs (recent):                                [L] View (follow)   │
│ [12:41:03] info handshake ok (180ms)                              │
└───────────────────────────────────────────────────────────────────┤
Hints: E env • R restart • L logs • b back


Behaviors: Tab navigation, inline actions (e.g., [E]), health metrics.
Widget: ratatui::widgets::Tabs, Paragraph, Table.


Add Plugin

Purpose: Install plugins (npm, pip, binary, http/sse).
Layout (Wizard):┌ Add Plugin ─────────────────────────────────────────┐
│ Method: (•) From npm   ( ) From pip   ( ) From command   ( ) Remote │
│ Name: github                                                     ⓘ │
│ Command: npx -y @modelcontextprotocol/server-github              ⓘ │
│ Env: GITHUB_TOKEN = ${secret:GITHUB_TOKEN}                          │
│ [Validate]  [Next]  [Cancel]                                        │
└────────────────────────────────────────────────────────────────────┘


Behaviors: Validates via --version or handshake, previews mcp.json patch.
Widget: ratatui::widgets::Block, Paragraph.


Logs Drawer

Purpose: View stdout/stderr with copy/export.
Layout:┌ Logs — github [p] pager  [y] copy line  [Y] copy all ──────┐
│ [12:41:03] info handshake ok (180ms)                              │
│ [12:42:10] info tool invoked: list_repos                          │
└───────────────────────────────────────────────────────────────────┘


Behaviors: Follow mode (f), search (/), OSC52 clipboard.
Widget: ratatui::widgets::List.


Environment Editor

Purpose: Edit environment variables with masking.
Layout:┌ Edit Env — github ──────────────────────────────────────────┐
│ KEY            VALUE (masked)         Source       Effective       │
├────────────────────────────────────────────────────────────────────┤
│ GITHUB_TOKEN   •••••••••••••••       secret        ✓               │
│ USER_AGENT     heroku-mcp/2.1        file          ✓               │
└────────────────────────────────────────────────────────────────────┤
Hints: Ctrl-S save • Esc cancel


Widget: ratatui::widgets::Table.

4. Behaviors and Workflows
4.1 Plugin Lifecycle

Start: Lazy (on first use) or auto (on TUI open); default 20s timeout for handshake.
Restart: On failure (exponential backoff); manual via [R].
Isolation: Stdio plugins run out-of-process; http/sse are remote.
Health: Monitor start time, handshake latency; display in Details.

4.2 Installation

Methods: npm (npx), pip (python -m), binary, http/sse.
Example (npm):{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "${secret:GITHUB_TOKEN}" }
    }
  }
}


Validation: Run --version or handshake; preview mcp.json changes.

4.3 UX Features

Search: / filters names/tags; highlights in teal.
Logs: Follow mode, copy/export (OSC52 fallback).
Toasts: Green (success), yellow (warn), red (error); auto-dismiss.
Errors: Show in Logs (e.g., "[err: handshake timeout]"); recovery actions (restart).
Authorization: Managed externally via RBAC/ABAC; TUI assumes plugins handle access control.

5. Implementation Notes
5.1 Rust and Ratatui

Components: List, Details, Logs, Environment Editor, Add Wizard, Toasts.
Rendering: Use ratatui::widgets (Table, List, Paragraph, Block); avoid blocking I/O.
State: TEA (The Elm Architecture) for event handling; async channels for logs.
Clipboard: System → OSC52 → file fallback.

5.2 Data Model
struct PluginDetail {
    name: String,
    status: Status, // Running | Stopped | Warn
    command_or_url: String,
    env: Vec<EnvVar>,
    logs: LogRingBuffer,
}

struct EnvVar {
    key: String,
    value: String, // Masked for secrets
    source: String, // file | secret | env
}

5.3 Security

Secrets: Mask in UI/logs; resolve via keyring-rs.
Audit Log: JSONL at ~/.config/heroku/mcp-audit.jsonl (e.g., {"server": "github", "action": "start", "time": "2025-09-07T15:55:03Z"}); rotate after 7 days or 10MB.
Permissions: Handled externally; no TUI enforcement.

5.4 Accessibility

Monochrome Mode: Disable colors; use bold/icons.
Keyboard-First: Focus rings, no mouse reliance.
Icons: ✓ (ok), ✗ (stopped), ! (warn) for clarity.
