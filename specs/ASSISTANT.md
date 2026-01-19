# ASSISTANT.md — Agent Integration via Lightweight MCP Server (Phase II)

**Owner:** TUI/Engine Team  
**Audience:** DX engineering, Platform, Security  
**Status:** Draft (v0.1)  
**Purpose:** Describe how the Phase II **Agent** integrates with the TUI via a **lightweight MCP server** that exposes safe, typed tools for command & schema discovery, validation, planning, and optional execution gating. The goal is to let users **generate or refine workflows** with the Agent while keeping the **TUI/engine as the source of truth** for preview, diffs, and runs.

---

## 1) Why an MCP Server?

- **Separation of concerns.** The Agent calls MCP **tools**; the TUI/engine keeps authority over rendering, diffing, and execution.  
- **Security & guardrails.** Expose only **read/plan/validate** surfaces to the Agent. Keep secrets and execution **out-of-band** and **consent-gated**.  
- **Portability.** The same MCP surface can be reused in the TUI, headless automation, or IDE plugins (Cursor, etc.).  
- **Determinism.** Outputs are **validated** against contracts and schemas before the TUI proposes a change.

---

## 2) Scope & Principles

- **The Agent generates/edits workflows** but **does not execute** them without an explicit, local confirmation step in the TUI.  
- **Typed contracts everywhere.** Tools return **CommandSpec** & **ProviderContract** (incl. `output_contract`, `args.accepts/prefer/required`, and `returns.fields`).  
- **One source of truth.** The TUI persists, previews, diffs, and runs workflows; the Agent drafts and validates.  
- **Safe defaults.** Prefer IDs over names where provider contracts say `prefer: id`. Redact obvious secrets.  
- **Minimal surface.** Start with search/describe/validate/dry‑run. Add recording→scaffold and execution later.

---

## 3) MCP Tool Surface (MVP)

> All tools are **read-only** except the optional, gated `execute`. Tool names and payloads are illustrative; keep JSON-minimal and stable.

### 3.1 `search_commands`
Fuzzy search across command registry.

**Input**
```json
{ "query": "create app postgres" }
```

**Output**
```json
{
  "results": [
    { "id": "apps:create", "group": "apps", "name": "create", "summary": "Create an app" },
    { "id": "addons:create", "group": "addons", "name": "create", "summary": "Attach add-on" }
  ]
}
```

### 3.2 `get_command_spec`
Return the full `CommandSpec` including **output_contract**.

**Input**
```json
{ "id": "apps:create" }
```

**Output (excerpt)**
```json
{
  "id": "apps:create",
  "path": "/apps",
  "method": "POST",
  "flags": [{ "name": "region", "type": "string" }],
  "positionals": [],
  "output_contract": {
    "fields": [
      { "name": "id", "type": "string", "tags": ["app_id"] },
      { "name": "name", "type": "string", "tags": ["app_name", "display"] }
    ]
  }
}
```

### 3.3 `list_providers`
List providers with **arg-contracts** and **returns.fields**.

**Output (excerpt)**
```json
{
  "providers": [
    {
      "id": "addons:list",
      "args": {
        "app": { "accepts": ["app_id", "app_name"], "prefer": "app_id", "required": true }
      },
      "returns": [
        { "name": "id", "tags": ["addon_id"] },
        { "name": "name", "tags": ["addon_name", "display"] }
      ]
    }
  ]
}
```

### 3.4 `validate_workflow`
Schema + contract validation with line/column ranges for TUI highlights.

**Input**
```json
{ "yaml": "<raw workflow yaml string>" }
```

**Output**
```json
{
  "errors": [
    { "message": "addons:list.app must accept one of [app_id, app_name]", "line": 22, "column": 18 }
  ],
  "warnings": [
    { "message": "select.id_field recommended for caching", "line": 14, "column": 7 }
  ]
}
```

### 3.5 `dry_run`
Create a **plan** by resolving templates and provider-arg mappings **without network calls**.

**Input**
```json
{ "yaml": "<raw workflow yaml string>", "sample_inputs": { "region": "us" } }
```

**Output (excerpt)**
```json
{
  "plan": {
    "inputs_required": ["pipeline", "app", "addon"],
    "steps": [
      {
        "id": "add_pg",
        "provider_args": { "addons:list.app": "steps.create_app.output.id" },
        "notes": ["prefer app_id per contract"]
      }
    ]
  }
}
```

### 3.6 `render_yaml`
Deterministically pretty-print an AST/draft back into YAML for diffing.

**Input**
```json
{ "draft": { "workflow": "app_with_db", "inputs": [], "steps": [] } }
```

**Output**
```json
{ 
  "yaml": "workflow: app_with_db",
  "inputs": [],
  "steps": []
}
```

### 3.7 *(Optional, gated)* `execute`
Run a full workflow or a single step. **Requires a session nonce** and is always **prompt‑confirmed** by the TUI.

---

## 4) Data Contracts (Essentials)

- **CommandSpec**: group, name, path, method, flags/positionals, **output_contract.fields[{name,type?,tags[]}]**  
- **ProviderContract**: `args.{name:{accepts[],prefer?,required?}}`, `returns.fields[{name,type?,tags[]}]`  
- **DraftWorkflow**: `workflow, description?, inputs[], steps[], diagnostics?`  
- **DryRunPlan**: `inputs_required[], steps[{id, provider_args, notes[]}]`

> Version each with `schema_version` and apply lenient parsing (unknown fields ignored) to allow forward-compatible evolution.

---

## 5) Agent Workflow Generation — Reference Flow

1. **Search**: `search_commands("create app postgres")` → shortlist.  
2. **Describe**: `get_command_spec` for targets; fetch `list_providers` for dependent inputs.  
3. **Scaffold**: Agent composes a `DraftWorkflow` (inputs + steps).  
4. **Validate**: `validate_workflow` → fix with contract-aware hints.  
5. **Dry‑run**: `dry_run` → show plan (resolved provider args, remaining required inputs).  
6. **Render**: `render_yaml` → TUI opens **diff+preview**.  
7. **User edits** (YAML or Form).  
8. **(Optional) Execute**: gated confirmation in TUI.

---

## 6) Security & Safety

- **No secrets to the Agent.** Tools return **shapes, IDs, tags**, not credentials or live values.  
- **Execution gating.** `execute` requires: user confirmation, a **session nonce**, and a summary of effects.  
- **Redaction.** TUI/engine redact tokens, passwords, and env-like strings from logs and previews.  
- **Rate limiting.** Bound tool QPS; cache stable responses (command specs, provider lists).  
- **Auditability.** All tool calls are logged with request/response summaries (without sensitive data).

---

## 7) UX Integration

- The **Editor** remains central (YAML/Form toggle).  
- The Agent proposes drafts; the TUI provides **Live Validation**, **Dry-run**, **Field Picker**, and **Diff/Save**.

## 8) Source Alignment

- **MCP infrastructure** lives in `crates/mcp/src/lib.rs`, with `plugin/engine.rs` orchestrating plugin processes and `provider/mcp_provider.rs` exposing tools as registry-backed value providers.
- **Configuration loading and reloads** are handled in `crates/mcp/src/config/mod.rs`; the UI calls `PluginEngine::update_config` whenever the config watcher in `crates/tui/src/ui/runtime.rs` fires.
- **TUI integration** wires the engine into `App::new` (`crates/tui/src/app.rs`) and dispatches MCP-backed effects through `crates/tui/src/cmd.rs` so Agent-issued commands share the same execution path as palette runs.
- **Schema contracts** referenced above map directly to the definitions in `crates/types/src/lib.rs` (`CommandSpec`, `ProviderContract`, `DraftWorkflow`, and `DryRunPlan`), ensuring the Agent and UI consume the same typed models.
- Explanations re-use the same “why” strings: `accepts`, `prefer`, and tag badges (`app_id`, `app_name`).

**UI hooks:**
- “Ask the Agent” action → opens a prompt panel; responses land as a proposed draft (side‑by‑side diff).  
- “Record → Propose” leverages a later tool (`infer_workflow_from_session`) to transform captured runs into a draft.

---

## 8) MVP vs. Phase II

**MVP (ship first):**
- Tools: `search_commands`, `get_command_spec`, `list_providers`, `validate_workflow`, `dry_run`, `render_yaml`.  
- TUI: prompt → draft → validate → preview → save.

**Phase II Enhancements:**
- `infer_workflow_from_session(session_log)`  
- Provider autocomplete while editing (`suggest_providers`)  
- Optional `execute` with consent  
- Template library & snippets; test fixtures for dry‑run

**Non‑Goals (for now):**
- Arbitrary remote execution without consent  
- Direct network calls from the Agent  
- Complex multi‑tenant auth flows within the MCP server

---

## 9) Architecture Sketch

```
┌──────────┐        MCP (tools)         ┌───────────────┐
│  Agent   │ ─────────────────────────► │  MCP Server   │
│ (LLM)    │                            │ (lightweight) │
└──────────┘                            └──────┬────────┘
                                              │
                    read/search/validate      │
                                              ▼
                                      ┌──────────────┐
                                      │ Registry     │
                                      │ (commands,   │
                                      │ providers,   │
                                      │ contracts)   │
                                      └─────┬────────┘
                                            │ plan/dry-run/execute (gated)
                                            ▼
                                      ┌──────────────┐
                                      │ Engine + TUI │
                                      │ (preview,    │
                                      │ diff, run)   │
                                      └──────────────┘
```

---

## 10) Operational Notes

- **Latency budget:** Keep tool calls < 150ms P50 by caching immutable specs in-memory.  
- **Error modes:** Tools return typed errors; the TUI shows actionable fixes (e.g., “use app_id — contract prefers id”).  
- **Testing:**  
  - Unit tests for validation and dry-run resolutions.  
  - Golden tests for `render_yaml`.  
  - Contract snapshots per release (diff highlights when APIs evolve).  
- **Versioning:** Include `schema_version` in all tool responses; the TUI/Agent should accept older minor versions.

---

## 11) Open Questions

1. Should `render_yaml` also normalize key ordering (e.g., inputs/steps) to minimize diffs?  
2. Do we support per‑provider pagination discovery (`{items,next_cursor}`) in `list_providers`?  
3. How much of the **recording → scaffold** pipeline should live server‑side vs. Agent‑side?  
4. Is a workspace‑level policy needed to disable `execute` in certain environments?

---

## 12) TL;DR

Provide the Agent a **lightweight MCP server** with **search/describe/validate/dry‑run/render**.  
Let the Agent **generate high‑quality workflows** from this typed surface, while the **TUI handles preview, diffs, and execution with explicit consent**.  
This delivers powerful automation with strong guardrails, clear explainability, and minimal coupling.
