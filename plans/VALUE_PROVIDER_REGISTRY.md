# ValueProvider Registry — Adapted for the New Command Spec

This document defines a **portable registry format** (YAML/JSON) to map **commands/flags/positionals** to **ValueProviders**. It lets the CLI/TUI discover which parameters can source **dynamic suggestions** (apps, addons, regions, etc.) from **core providers**, **workflow outputs**, or **MCP plugins**.

**manifest entry**
```json
{
  "group": "enterprise-accounts",
  "name": "members:create",
  "summary": "Create a member in an enterprise account.",
  "positional_args": ["enterprise_account"],
  "positional_help": {
    "enterprise_account": "unique identifier of the enterprise account or unique name of the enterprise account"
  },
  "flags": [
    { "name": "federated", "short_name": "f", "required": false, "type": "boolean", "enum_values": [], "default_value": null, "description": "whether membership is being created as part of SSO JIT" },
    { "name": "permissions", "short_name": "p", "required": true, "type": "array", "enum_values": [], "default_value": null, "description": "permissions for enterprise account" },
    { "name": "user", "short_name": "u", "required": true, "type": "string", "enum_values": [], "default_value": null, "description": "unique email address of account or unique identifier of an account" }
  ],
  "method": "POST",
  "path": "/enterprise-accounts/{enterprise_account}/members"
}
```

---

## 0) Versioning

- **Format id:** `vp-registry@1` (unchanged)
- **Registry semver:** `version: 1.1.0` (new minor to reflect the mapping layer)
- Backward-compatible with prior provider definitions; only **`commands`** mapping changed.

---

## 1) Field Addressing Model

With the new command spec, fields are addressed as:

- **Command identity:** `group` + `name` 
  - Example key → `enterprise-accounts members:create`
- **Positionals:** by name from `positional_args`, e.g., `enterprise_account`
- **Flags:** by **long name** (from `flags[].name`) rendered as `--<name>` in CLI; short names (e.g., `-u`) are aliases
- **Types:** from `flags[].type` (`string|boolean|array|int|json|…`)

**Registry rule:** Always reference a field by:
- `kind: "positional"` + `name: "<positional_name>"`, or  
- `kind: "flag"` + `flag: "--<flag_name>"` (short names are optional metadata)

---

## 2) Registry Structure (YAML)

```yaml
format: vp-registry@1
version: 1.1.0

defaults:
  ttl_seconds: 60
  debounce_ms: 180
  max_items: 50
  security:
    redact_patterns: ["token", "password", "secret", "authorization", "database_url"]
  behaviors:
    partial_required: true
    incremental: true
    cache_scope: "session"

providers:
  # Core dynamic providers
  - id: enterprise:accounts
    kind: core
    description: List enterprise accounts accessible to the user
    input:
      params: []
    output:
      item:
        label: "$.name"         # visible label
        value: "$.id"           # what to insert (or "$.slug"/"$.name" depending on API)
        meta:  "owner: $.owner.email"
    ttl_seconds: 120
    endpoint: "GET /enterprise-accounts"
    requires_auth: true

  - id: enterprise:permissions
    kind: core
    description: List valid enterprise permissions
    input:
      params:
        - name: enterprise_account
          from: "argOrFlag"     # from positional or flag (if mirrored)
    output:
      item:
        label: "$.name"
        value: "$.name"
        meta:  "$.description"
    ttl_seconds: 300
    endpoint: "GET /enterprise-accounts/{enterprise_account}/permissions"
    requires_auth: true

  - id: accounts:lookup
    kind: core
    description: Suggest user accounts by email or id
    input:
      params:
        - name: partial
          from: "partial"       # user-typed prefix
    output:
      item:
        label: "$.email"
        value: "$.id"
        meta:  "name: $.name"
    ttl_seconds: 45
    endpoint: "GET /accounts?query={partial}"
    requires_auth: true

  # Workflow outputs (virtual)
  - id: workflow:from
    kind: workflow
    description: Read a value from a prior task's JSON output
    input:
      params:
        - name: task
        - name: jsonpath
    output:
      item: { label: "$", value: "$" }
    ttl_seconds: 0
    requires_auth: false

commands:
  # Command key = "<group> <name>"
  - key: "enterprise-accounts members:create"
    fields:
      # positional: enterprise_account
      - name: enterprise_account
        kind: positional
        provider: enterprise:accounts
        behavior:
          partial_required: true
          debounce_ms: 120
        # If the API accepts either id or slug, choose insertion:
        output_value: "$.id"          # override provider default for this field (optional)

      # flag: --user (string; required)
      - name: user
        kind: flag
        flag: --user
        provider: accounts:lookup
        behavior:
          partial_required: true
          incremental: true

      # flag: --permissions (array; required)
      - name: permissions
        kind: flag
        flag: --permissions
        provider: enterprise:permissions
        args:
          enterprise_account: "${{ field.enterprise_account || context.enterprise_account }}"
        behavior:
          partial_required: false      # show full list; filter locally
          max_items: 200
```

### Notes
- `output_value` (optional) lets you override what from the provider item is inserted: e.g., insert `id` while displaying `name`.
- For **array flags** like `--permissions`, suggestions return one value at a time; the input accumulates multiple values (`--permissions admin --permissions read`), or a CSV if your parser supports it.
- `args.enterprise_account` demonstrates **templating** pulling from the positional `enterprise_account` or a global context.

---

## 3) JSON Equivalent (compact)

```json
{
  "format": "vp-registry@1",
  "version": "1.1.0",
  "providers": [
    {
      "id": "enterprise-accounts list",
      "kind": "core",
      "description": "List enterprise accounts accessible to the user",
      "input": { "params": [] },
      "output": { "item": { "label": "$.name", "value": "$.id", "meta": "owner: $.owner.email" } },
      "ttl_seconds": 120,
      "endpoint": "GET /enterprise-accounts",
      "requires_auth": true
    },
    {
      "id": "enterprise-permissions list",
      "kind": "core",
      "description": "List valid enterprise permissions",
      "input": { "params": [ { "name": "enterprise_account", "from": "argOrFlag" } ] },
      "output": { "item": { "label": "$.name", "value": "$.name", "meta": "$.description" } },
      "ttl_seconds": 300,
      "endpoint": "GET /enterprise-accounts/{enterprise_account}/permissions",
      "requires_auth": true
    },
    {
      "id": "accounts:lookup",
      "kind": "core",
      "description": "Suggest user accounts by email or id",
      "input": { "params": [ { "name": "partial", "from": "partial" } ] },
      "output": { "item": { "label": "$.email", "value": "$.id", "meta": "name: $.name" } },
      "ttl_seconds": 45,
      "endpoint": "GET /accounts?query={partial}",
      "requires_auth": true
    },
    { "id": "workflow:from", "kind": "workflow", "description": "Read a value from a prior task's JSON output",
      "input": { "params": [ { "name": "task" }, { "name": "jsonpath" } ] },
      "output": { "item": { "label": "$", "value": "$" } },
      "ttl_seconds": 0, "requires_auth": false }
  ],
  "commands": [
    {
      "key": "enterprise-accounts:members:create",
      "fields": [
        { "name": "enterprise_account", "kind": "positional",
          "provider": "enterprise:accounts",
          "behavior": { "partial_required": true, "debounce_ms": 120 },
          "output_value": "$.id" },
        { "name": "user", "kind": "flag", "flag": "--user",
          "provider": "accounts:lookup",
          "behavior": { "partial_required": true, "incremental": true } },
        { "name": "permissions", "kind": "flag", "flag": "--permissions",
          "provider": "enterprise:permissions",
          "args": { "enterprise_account": "${{ field.enterprise_account || context.enterprise_account }}" },
          "behavior": { "partial_required": false, "max_items": 200 } }
      ]
    }
  ]
}
```

---

## 4) Resolution & Templating (with the new spec)

**Resolution precedence for arguments passed to providers:**
1. `field.<positional_or_flag>` — current values entered by the user  
   - Positionals: `field.enterprise_account`  
   - Flags: `field.--user`, `field.--permissions`  
2. `context.*` — host-provided (e.g., default enterprise)  
3. `tasks.<name>.output.*` — workflow outputs (when executing a workflow)  
4. Explicit `args` in the registry mapping (templated strings)

**Templating examples:**
- `"${{ field.enterprise_account || context.enterprise_account }}"`
- `"${{ tasks.bootstrap.output.owner_email }}"`

---

## 5) UX Behavior (Power & Guided Modes)

- **Power Mode (command line)**
  - When cursor is on `enterprise_account` (positional #1), popup suggests from `enterprise:accounts`.
  - On `--user` value, popup suggests from `accounts:lookup` as you type (debounced prefix).
  - On `--permissions`, shows a list from `enterprise:permissions`; selection appends into the array value.

- **Guided Mode (form)**
  - `enterprise_account`: searchable dropdown.
  - `user`: autocomplete text field with email → id mapping.
  - `permissions`: multi-select chips or checklist; validation ensures at least one.

- **Insertion policies**
  - `enterprise_account` inserts the **id** (per `output_value: "$.id"`), even if label shows `name`.
  - `--user` inserts account **id** though label shows email (auditability + uniqueness).
  - `--permissions` inserts selected permission names one-by-one.

---

## 6) JSON Schema Updates (commands mapping only)

> This extends the previous registry schema; only the command mapping is shown here.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.com/schemas/vp-registry@1.commands-v2.json",
  "type": "object",
  "required": ["format","version","providers","commands"],
  "properties": {
    "format": { "const": "vp-registry@1" },
    "version": { "type": "string" },
    "providers": { "type": "array" },
    "commands": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["key","fields"],
        "properties": {
          "key": { "type": "string", "description": "group:name e.g., enterprise-accounts:members:create" },
          "fields": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["name","kind"],
              "properties": {
                "name": { "type": "string" },
                "kind": { "enum": ["flag","positional"] },
                "flag": { "type": "string", "pattern": "^--[a-z0-9][a-z0-9-]*$" },
                "required": { "type": "boolean" },
                "provider": { "type": "string" },
                "args": { "type": "object", "additionalProperties": {} },
                "behavior": {
                  "type": "object",
                  "properties": {
                    "debounce_ms": { "type": "integer", "minimum": 0 },
                    "max_items": { "type": "integer", "minimum": 1 },
                    "partial_required": { "type": "boolean" },
                    "incremental": { "type": "boolean" }
                  }
                },
                "output_value": { "type": "string", "description": "JSONPath for value to insert" }
              },
              "allOf": [
                { "if": { "properties": { "kind": { "const": "flag" } } }, "then": { "required": ["flag"] } }
              ],
              "additionalProperties": true
            }
          }
        }
      }
    }
  }
}
```

---

## 7) Provider Kinds & Invocation (unchanged)

- **core**: host HTTP clients, obey TTLs and debounce.
- **workflow**: read from in-memory task outputs (no I/O).
- **mcp**: delegate to plugin autocomplete; cache + timeouts.
- **static**: schema enums / literals (implied providers).

---

## 8) Edge Cases & Policies

- **Array flags (`--permissions`)**:
  - Each acceptance inserts a single item; users can select multiple times.
  - In Guided mode, use a multi-select UI; in Power mode, accumulate multiple `--permissions` occurrences.
- **Boolean flags (`--federated`)**:
  - No provider. Optionally support `--no-federated` if you expose negation.
- **Enterprise account identifier**:
  - If API supports **name or id**, decide insertion via `output_value`.
  - If you need **both** (display name, send id), registry handles it via label/value mapping.

---

## 9) Example: End-to-End Flow

1. User starts typing:
   ```
   enterprise-accounts:members:create <cursor>
   ```
   Popup shows enterprise accounts from `enterprise:accounts`.

2. User selects “Acme Corp” (id: `ea_123`):  
   The positional `enterprise_account` inserts `ea_123`.

3. User types `--user` and partial `jus` → suggestions from `accounts:lookup` return `justin@acme.com (id acc_42)`; acceptance inserts `acc_42`.

4. User adds `--permissions` → list appears from `enterprise:permissions` for `ea_123`. They pick `admin` and `read`.

5. Final command line (Power Mode):
   ```
   enterprise-accounts:members:create ea_123 --user acc_42 --permissions admin --permissions read
   ```

---

## 10) Migration & Authoring Tips

- **Key normalization**: Always compute command key as `group:name`.
- **Short flags**: Short names (`-u`) are derived from command spec; registry always references long form (`--user`).
- **Display vs value**: Prefer labels for humans (`name`, `email`) but **insert stable identifiers** (`id`) using `output_value`.
- **Performance**: Use `partial_required: true` for large lists (accounts, users); leave it `false` for constrained enums (permissions).

---

This adaptation aligns the ValueProvider system with your new **command spec** while preserving all prior capabilities (async providers, caching, templating, workflows, and MCP extensibility). Use this as a working spec to wire up dynamic suggestions for `enterprise-accounts:members:create` and beyond.
