# TUI Inline Editing & Autosave — UX Specification

## Scope
This specification defines the interaction contract for **inline editable fields**, **checkbox toggles**, **row selection**, and **autosave behavior** in a keyboard-first Terminal User Interface (TUI) with a master–detail layout.

The goal is to ensure:
- Predictable keyboard behavior
- Minimal cognitive load
- No hidden or ambiguous state
- Safe, reversible operations

---

## Core Principles

1. **Selection drives context**  
   The selected row defines the active entity and (by default) the details panel content.

2. **Navigation, mutation, and commitment are distinct intents**  
   - Navigation = moving focus
   - Mutation = changing state or text
   - Commitment = persisting a change

3. **Autosave is implicit but never ambiguous**  
   Users are never required to click "Save", but must always receive clear feedback.

4. **Destructive actions require explicit intent**  
   Accidental input must never silently mutate persistent state.

---

## Interaction Modes

### 1. Navigation Mode (Default)

- Purpose: Browse and inspect
- Active when no field is being edited

**Allowed actions:**
- Move selection
- Toggle checkboxes
- Trigger primary actions

---

### 2. Edit Mode (Inline Editing)

- Purpose: Modify a single editable field
- Explicitly entered
- Visually distinct (caret visible, field styling changes)

**Rules:**
- Text input mutates only while in edit mode
- Exiting edit mode always results in either **commit** or **cancel**

---

## Keybinding Contract

### While in Navigation Mode

| Key | Behavior |
|---|---|
| ↑ / ↓ | Move selection; update details panel |
| Mouse row click | Select row; update details panel |
| Space | Toggle checkbox; select row; **do not update details panel** |
| Mouse checkbox click | Select row + toggle; **do not update details panel** |
| Enter | Invoke primary action (open, drill-in, expand) |
| Tab / Shift+Tab | Move focus between editable fields (no mutation) |

---

### While in Edit Mode

| Key | Behavior |
|---|---|
| Enter | **Commit edit**, exit edit mode, stay on field |
| Tab | **Commit edit**, exit edit mode, move to next editable field |
| Shift+Tab (Backtab) | **Commit edit**, exit edit mode, move to previous editable field |
| Esc | **Cancel edit**, revert value, exit edit mode |

**Important:**
- Tab / Backtab must *never* cancel edits
- Cancel is explicit and bound only to Esc

---

## Autosave Behavior

### Checkboxes / Toggles

- Persist immediately (optimistic update)
- Must be reversible
- On failure:
  - Revert state
  - Display error in status/footer
  - Maintain selection

---

### Editable Fields

- Autosave occurs on **commit**, not per keystroke
- Commit triggers:
  - Enter
  - Tab / Backtab
  - Explicit navigation away *after* commit

**Failure handling:**
- Remain in edit mode
- Preserve user input
- Display inline error
- Do not advance focus

---

## Details Panel Update Rules

| Action | Details Panel Updates |
|---|---|
| Selection via navigation | Yes |
| Row click | Yes |
| Checkbox toggle (keyboard or mouse) | No |
| Edit commit | Yes |

Rationale: prevent visual churn during operational tasks while preserving inspection flow.

---

## Visual Requirements

### Edit Mode Indicators
- Visible text caret
- Field-level style change (background, underline, or inverse)
- Optional status hint:
  ```
  Editing — Enter save • Tab next • Esc cancel
  ```

### Save Feedback
- Quiet, non-blocking acknowledgment
- Examples:
  - Status line message
  - Brief checkmark indicator

---

## Non-Goals

- No global Save / Apply button
- No autosave on/off toggle
- No hidden dirty state
- No multi-step commit flows

---

## Summary Contract (Canonical)

- **Selection = context**
- **Space = toggle**
- **Enter = commit or primary action**
- **Tab = commit and move**
- **Esc = cancel**
- **Autosave is default, explicit cancel is required**

This contract must remain consistent across all screens to preserve user trust and muscle memory.
