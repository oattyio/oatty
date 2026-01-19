# Library Details Pane UX Spec (Oatty CLI TUI)

## 1) Goals and Scope
- Provide a focused details panel for the selected `RegistryCatalog`.
- Present a skimmable, paragraph-based summary of the `RegistryManifest`.
- Allow editing only the catalog fields that are user-tunable: `base_url` selection and `headers`.
- Mirror existing UI patterns: command palette-style completions for base URLs and key/value table editing for headers.

## 2) Data Surfaces and Copy Rules
### 2.1 RegistryCatalog fields displayed
- Name, description, manifest path.
- Base URLs (count + selected URL).
- Headers (count + redaction of sensitive values).
- Manifest load status (present or missing).

### 2.2 RegistryManifest summary paragraphs
Render two short paragraphs separated by a blank line. Each paragraph should be 1-2 sentences with leading labels for quick scanning.

Example copy when manifest exists:
- Paragraph 1: "Vendor: Acme Corp. 128 commands and 6 workflows available."
- Paragraph 2: "Providers: 42 provider contracts defined. 31 commands rely on provider-backed inputs."

Example copy when manifest is missing:
- Paragraph 1: "Vendor: Unknown. Manifest not loaded for this catalog."
- Paragraph 2: "Providers: None (load a manifest to see contracts)."

Notes:
- Use `commands.len()`, `workflows.len()`, and `provider_contracts.len()`.
- If vendor is empty, use "Unknown".
- Omit any data not present in `RegistryManifest`.

## 3) TUI Layouts (Annotated ASCII)
### 3.1 Details pane
```
+─ Catalog Detail ────────────────────────────────────────────────+
│ Import   Remove                                                 │
│ Catalogs                                                        │
│ [v] Render Public API   enable                                  │
+─────────────────────────────────────────────────────────────────+
│ Render Public API (enabled)                                     │
│ Command Prefix: render                                          │
│ Endpoints: 193      Workflows: 0      Value providers: 177      │
│ Description: Manage everything about your Render services       │
│─────────────────────────────────────────────────────────────────│
│ Active base URL                                 + Add  - Remove │
│   ⓧ                                              (error chip)  │
│  (●) https://api.render.com/v1                                 │
│─────────────────────────────────────────────────────────────────│
│ Vimeo API Headers                              + Add  - Remove  │
│                                              [ ] Show secrets   │
│ Header                             Value (optional)             │
│ ─────────────────────────────────────────────────────────────── │
+─────────────────────────────────────────────────────────────────+
```

### 3.2 Inline editors
- Selecting **Add** in the base URL section opens the palette-style input directly inline (no full-pane edit mode), with the text field replacing the current radio list until a value is committed or cancelled with `Esc`.
- Selecting **Add** in the headers section appends a new editable row at the top of the table, focusing the key column and following the inline editing contract from §6.

## 4) Interaction Model
### 4.1 Summary + stats
- The header block always shows the selected catalog name, enablement state, command prefix, endpoint/workflow/provider counts, and description. These fields update live when the catalog is toggled.
- Pressing `Enter` while the catalog list on the left is focused keeps the detail pane in sync; there is no separate summary modal.

### 4.2 Base URLs
- The section is always interactive: radio buttons select the active base URL.
- `+ Add` opens the palette-style editor inline (see §5) and seeds it with the currently selected base URL.
- `- Remove` deletes the highlighted base URL after confirming (reuse the confirmation modal if multiple URLs exist; otherwise disable the control).
- A leading red `x` badge appears when validation fails (missing base URL, invalid scheme, etc.).

### 4.3 Headers
- `+ Add` inserts a new row and focuses the key column.
- `- Remove` deletes the currently highlighted row; disable when no headers exist.
- `[ ] Show secrets` toggles redaction for the value column (defaults to redacted). When revealed, emit a toast warning per security guidelines.
- Rows are editable in place; there is no separate edit mode.

## 5) Base URL Editor (Palette-Style Completions)
### 5.1 Layout
- When the user chooses `+ Add` or presses `Enter` on an existing radio option, replace the radio list with:
  - A single-line input styled like the command palette.
  - A suggestion list below it populated from `catalog.base_urls` plus recents.
  - Footer hints inline with the section title (`Enter add • Esc cancel`).

### 5.2 Behavior
- Character input filters suggestions by substring (case-insensitive).
- `Tab` opens the suggestions list if hidden.
- `Up/Down` moves the selection in the list.
- `Enter` accepts the selected suggestion (creating it if it does not already exist) and restores the radio list with the new entry selected.
- `Esc` cancels editing and restores the previous list.
- Display a ghost text suffix for the top suggestion, following command palette behavior.

### 5.3 Validation and hints
- Basic URL validation: must be a valid scheme + host (use the existing URL parser).
- Errors appear as a red badge (`ⓧ`) above the input and a single-line message beneath it; keep the field focused until the value is fixed or cancelled.

## 6) Headers Editor (Key/Value Table)
### 6.1 Layout
- Use the same table and inline editor pattern as `KeyValueEditorComponent`.
- Table columns: `KEY` and `VALUE`.
- Inline editor appears below the table when editing a row.

### 6.2 Behavior (mirrors kv_component)
- `Up/Down` moves selection.
- `Ctrl+N` adds a new row.
- `Ctrl+D` or `Delete` removes the selected row.
- `Enter` or `Ctrl+E` begins editing the selected row.
- `Tab` toggles between key/value fields while editing.
- `Enter` commits the edit; `Esc` cancels.

### 6.3 Redaction rules
- Render header values through `oatty_util::redact_sensitive`.
- Allow editing of redacted values; show actual input while editing.

## 7) Focus and Navigation
- The details pane uses its own focus ring with two focusable areas:
  - Base URL editor.
  - Headers editor.
- `Tab` and `Shift+Tab` move between base URL and headers.
- When exiting edit mode, return focus to the details pane (not the list).

## 8) Empty, Missing, and Error States
- No selected catalog: render centered text "Select a catalog to see details".
- Manifest missing: show the summary paragraphs with the "Vendor: Unknown" copy and a muted line "Manifest not loaded".
- Invalid manifest: show a short error line in the summary section; keep catalog details visible.

## 9) Implementation Notes
- Use `theme_helpers::block` and `theme.border_style(focused)` for all sections.
- Keep summary copy in a `Paragraph` with `Wrap { trim: true }` for clean line breaks.
- The details pane should never scroll the list on the left; it manages its own layout only.

## 10) Source Alignment

- **Component + state**: `crates/tui/src/ui/components/library/library_component.rs`, `state.rs`, and `types.rs` implement the detail pane, edit mode transitions, and registry catalog wiring described in this spec.
- **Base URL editor**: The palette-style single-line input and suggestion list reuse the palette helpers and focus flags defined in `library/state.rs`, so the copy above mirrors the actual Ratatui widgets.
- **Headers editor**: Inline editing behavior is backed by `crates/tui/src/ui/components/common/key_value_editor/`, which enforces the autosave/commit contract and redaction rules referenced in sections 6.2 and 6.3.
