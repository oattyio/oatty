# Heroku CLI — MCP Integration as "Plugins"

## Goals
- Leverages existing MCP apps.
- Safe execution in separate process.
- No main-thread crashes.

---

## Plugin Model
- Plugins run as **separate processes**.
- Communication via **stdio (JSON-RPC)**.
- CLI orchestrates commands, results, and UI integration.

---

## MCP Integration
- Plugins expose tools using MCP schema.
- Host CLI:
  - Calls `list_tools`.
  - Executes tool requests.
  - Displays results in TUI (tables, detail drawers).
- Plugins can reuse **existing MCP apps** with no rewrite.

---

## Shell Requests
Plugins may request shell execution via PTY handoff:
```json
{
  "jsonrpc":"2.0",
  "method":"request_shell",
  "params":{
    "command":"psql",
    "args":["-h","db.example.com","-U","user","-d","demo"],
    "prefer":"suspend"
  }
}
```
- Host suspends TUI.
- Spawns process under PTY.
- Restores TUI on exit.

---

## Safety
- Process isolation.
- Host controls environment vars passed through.
- Misbehaving plugin cannot crash CLI.
