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
### 3.1 Details pane, view mode
```
+-- Details ---------------------------------------------------+
| Catalog: Acme Registry                                       |
| Manifest: Loaded from /path/to/manifest.bin                  |
| Base URLs: 2 (selected: https://api.acme.com)                |
| Headers: 3 (1 redacted)                                      |
|--------------------------------------------------------------|
| Manifest Summary                                             |
| Vendor: Acme Corp. 128 commands and 6 workflows available.   |
|                                                              |
| Providers: 42 provider contracts defined. 31 commands rely   |
| on provider-backed inputs.                                   |
|--------------------------------------------------------------|
| Configuration                                                |
| Base URL: https://api.acme.com                               |
| Headers: 3 entries (press E to edit)                         |
+--------------------------------------------------------------+
```

### 3.2 Details pane, edit mode
```
+-- Details (Edit) --------------------------------------------+
|                                                  [Save] [X]  |
| Catalog: Acme Registry                                       |
| Manifest: Loaded from /path/to/manifest.bin                  |
|--------------------------------------------------------------|
| Base URL                                                     |
| > https://api.acme.com                                       |
|   https://api.acme.com             (selected)                |
|   https://staging.acme.com                                   |
|   https://eu.acme.com                                        |
|--------------------------------------------------------------|
| Headers                                                      |
| KEY                         VALUE                            |
| Authorization              ***************                   |
| X-Client-Id                123456                            |
|--------------------------------------------------------------|
| Edit row 2 (value)                                           |
| Key:   X-Client-Id                                           |
| Value: 123456                                                |
+--------------------------------------------------------------+
```

## 4) Interaction Model
### 4.1 View mode
- Details are read-only.
- Press `E` to enter edit mode when the details pane has focus.
- Press `Enter` on the base URL row to quick-open edit mode and focus the base URL input.

### 4.2 Edit mode
- `Save` applies base URL selection and headers updates to the `RegistryCatalog`.
- `X` (or `Esc`) exits edit mode, discarding uncommitted edits.
- If a header row is actively edited, `Esc` first cancels the inline edit, then exits edit mode.

## 5) Base URL Editor (Palette-Style Completions)
### 5.1 Layout
- A single-line input at the top of the section.
- A suggestions list directly below, populated from `catalog.base_urls`.
- When the input is empty, show all base URLs with the currently selected item marked.

### 5.2 Behavior
- Character input filters suggestions by substring (case-insensitive).
- `Tab` opens the suggestions list if hidden.
- `Up/Down` moves the selection in the list.
- `Enter` accepts the selected suggestion.
- If the input does not match an existing URL, `Enter` adds it to `base_urls` and selects it.
- `Esc` closes the suggestions list (does not exit edit mode).
- Display a ghost text suffix for the top suggestion, following command palette behavior.

### 5.3 Validation and hints
- Basic URL validation: must be a valid scheme + host (use existing URL parser if available).
- Display errors inline under the input (muted/error style) without blocking typing.

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
