# WORKFLOW TUI — Collector UX Addendum (Guided Input Collector)

**Status:** Adopted  
**Scope:** Defines the behavior and UI for resolving provider-backed inputs in a **Guided Input Collector** modal, including keybindings, manual fallback, cached behavior, and persistence of user choices.

---

## 1) When to Launch the Collector

- **Trigger:** A workflow has **unresolved provider arguments** (e.g., dependent providers need values, validation marked `required`, or a provider fetch failed with `on_error`).  
- **Behavior:** Open a **Guided Input Collector** modal that walks the user through **all unresolved inputs** for the selected workflow.  
- **Inline edits:** The Inputs list still supports inline edits for quick tweaks; however, the collector provides a **focused, end-to-end flow** to achieve a resolvable state.

---

## 2) Layout & Flow

```
┌─ Guided Input Collector — Resolve Inputs ────────────────────────────────────────────────────────────────────────┐
│ Workflow: provision_and_promote  •  Unresolved: 3                                                                │
├─ Unresolved Inputs (sticky head) ───────────────────────────┬─ Details / Candidates ─────────────────────────────┤
│ 1) addon.app        (addons:list)  needs: [app_id|app_name] │ Provider: addons:list                              │
│ 2) pipeline         (pipelines:list)                        │ Arg contracts:                                     │
│ 3) prod_app         (apps:list)                             │   app: accepts [app_id,app_name]                   │
│                                                             │  prefer app_id  • required                         │
│ [↑/↓] select • [Enter] resolve • [/] filter • [r] refresh   │ Candidates (Page 1/2):                             │
│                                                             │ 1) steps.create_app.output.id                      │
│                                                             │    → "app-456" [tags: app_id]                      │
│                                                             │ 2) steps.create_app.output.name                    │
│                                                             │    → "billing-svc" [app_name]                      │
│                                                             │ 3) inputs.app → "billing-svc"                      │
│                                                             │Actions: [Enter] choose  [f] Picker pane  [/] Filter│
├─ Manual Entry (fallback) ───────────────────────────────────┴────────────────────────────────────────────────────┤
│ on_error: manual — Enter value for addon.app:  [________________________]  (validate: app_id|name)               │
│ [Tab] switch to candidates • [r] retry provider • [c] use cached (24s old)                                       │
├─ Footer ─────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ [Space] toggle multi-select  •  [Enter] apply  •  [Esc] close  •  [Alt+R] Remember this mapping                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Flow:**
1. User selects an unresolved field from the left list.  
2. The right pane shows **contracts** (accepts/prefer/required) and **schema-valid candidates** (with tag badges) by default, and can flip into the inline **Field Picker** tree on demand.  
3. The user can pick a candidate, toggle the inline **Field Picker** pane for full JSON browsing, or enter a **manual fallback**.  
4. Repeat until unresolved = 0, then the modal closes or offers **Run**/**Dry-run**.

---

## 3) Keybindings

- **Navigation:** `↑/↓` navigate unresolved fields; `/` filter; `r` refresh provider cache; `Enter` select/apply; `Esc` close.  
- **Multi-select lists:** `Space` toggles items.  
- **Field Picker pane:** `f` toggles the detail column between candidates and the picker tree; type to filter (or press `/` to clear), use `↑/↓` to move, `←/→` to collapse/expand, `Enter` to select, `Esc` returns to candidates.  
- **Optional:** `Alt+R` toggles **Remember this mapping** (see Persistence).

---

## 4) Manual Fallback

- **Single-surface UX:** If `on_error=manual` or a provider fails, render a **basic text input inline within the collector modal** for that field.  
- **Validation:** Apply known validation (e.g., enum, regex, accepts tags) and show immediate feedback.  
- **Context preserved:** Contracts and candidates remain visible alongside the input, avoiding context switches.

---

## 5) Cached Behavior

- For `on_error=cached` or when a cached response is available:
  - **Render cached results** with an **age/TTL badge** (e.g., “loaded 24s ago”).  
  - Provide **Retry** and **Manual** options alongside cached choices.  
  - **Do not lock** to cached-only; users may still manually override or retry.

---

## 6) Persistence of User Choices

- **Default:** Persist **for the current run/session** so reruns keep the same resolution without re-prompting.  
- **Optional, explicit:** Offer a **“Remember this mapping”** toggle per-field or per-workflow.  
  - Storage: lightweight local config/history (keyed by workflow + field + stable `id_field` where available).  
  - Include an expiration (TTL) to avoid stale mappings.  
  - Surfaced origin badges like `history` when auto-applied, with a key to clear.

---

## 7) Error & Ambiguity Handling

- **Contracts first:** Candidates are derived via **producer output contracts** (tags like `app_id`, `app_name`) and **provider arg-contracts** (`accepts`, `prefer`).  
- **Heuristics second:** If multiple remain after tag-based filtering, show ranked candidates and switch the detail pane to the **Field Picker** view upon request.  
- **Explainability:** Each candidate includes a short **“why”** (e.g., “matches accepts; prefer app_id”).

---

## 8) Accessibility & Internationalization

- **Keyboard-first** interactions; concise status lines announce loading/error/cached states.  
- Respect wide glyphs and right-to-left text in list/table cells.  
- All actions available via keys; on-screen hints rendered in the footer.

---

## 9) Telemetry (Optional)

- Emit counters for:
  - **Resolution path:** auto vs. picker vs. manual vs. cached.  
  - **Refresh outcomes:** provider success vs. fail, cache hit ratio.  
  - **Persistence use:** how often “Remember this mapping” is toggled.  
- No PII; reference `id_field` only when needed for stability.

---

## 10) Rationale Summary

- **Modal collector** focuses the user on resolving all blocking inputs, then returns them to normal flow.  
- **Inline fallback & cached controls** reduce context-switching and keep contracts visible.  
- **Session persistence + opt-in cross-session memory** balances convenience with safety, avoiding stale mappings by default.
