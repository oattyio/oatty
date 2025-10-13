Reusable Text Input (UTF-8 safe)

Context
- The command palette implements robust, UTF-8 safe cursor management and editing along with suggestion/ghost text behavior. Many other TUI surfaces need only the text/cursor primitives without suggestion/history.

What exists now
- A new module at crates/tui/src/ui/components/common/text_input.rs that encapsulates:
  - input: String and cursor: usize (byte index on a UTF‑8 boundary)
  - UTF‑8 safe primitives: move_left, move_right, insert_char, backspace
  - Basic setters/getters and optional token helper
- This mirrors the core palette editing behavior found in:
  - reduce_move_cursor_left/right, apply_insert_char, reduce_backspace
  - token_index_at_cursor/get_current_token pattern

Feasibility assessment
- High: The palette’s editing primitives are self-contained and reusable.
- Separation:
  - Keep suggestions/history/ghost text out of the reusable layer.
  - Provide just the text/cursor model; allow higher layers (palette, forms, etc.) to compose suggestions and ghost text as needed.
- Risk: Minimal. The module is additive and not wired into existing components yet.

Integration guidelines
- For inputs that accept free-form text (e.g., workflow input editors), embed TextInputState in their state structs and delegate key handling to it:
  - Left/Right arrows map to move_left/move_right
  - Character input maps to insert_char
  - Backspace maps to backspace
- If token awareness is needed (e.g., for highlighting or completions), keep lexing in the feature module and use token_at_cursor helper or your own ranged tokens.

Future enhancements
- Optional: Extract a shared ghost_remainder helper if other components adopt ghost text.
- Optional: Provide copy_with_space/insert_with_space helpers when composing completions.
- Optional: Provide history browsing as a separate reusable mixin/state.

Testing
- Unit tests in the module validate multi-byte correctness across movement, insertion, and deletion.
