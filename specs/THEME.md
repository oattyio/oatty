# Theme System (As-Built)

## Scope
This document describes the implemented theme system in:
- `crates/tui/src/ui/theme`

## Available Themes
Truecolor themes:
- Dracula (`dracula`)
- Nord (`nord`)
- Cyberpunk (`cyberpunk`)
- High-contrast variants for each

ANSI fallback themes:
- `ansi256`
- `ansi256_hc`

## Theme Selection
Selection order:
1. Terminal capability detection
2. `TUI_COLOR_MODE` / `TUI_FORCE_TRUECOLOR`
3. `TUI_THEME`
4. persisted preference passed into `theme::load`
5. default theme fallback

If terminal capability is ANSI-only, an ANSI palette is forced.

## Theme Contract
- Shared semantic roles are defined in `roles::ThemeRoles`.
- Components style through the `Theme` trait and `theme_helpers`.
- No component should hard-code visual colors outside theme modules.

## Common Helpers Used by Components
- `block`, `panel_style`, `table_*_style`
- `styled_line` for log-friendly highlighting
- button and badge helper styles

## Source Alignment
- `crates/tui/src/ui/theme/mod.rs`
- `crates/tui/src/ui/theme/roles.rs`
- `crates/tui/src/ui/theme/theme_helpers.rs`
- `crates/tui/src/ui/theme/catalog.rs`
- `crates/tui/src/ui/theme/dracula.rs`
- `crates/tui/src/ui/theme/nord.rs`
- `crates/tui/src/ui/theme/cyberpunk.rs`
- `crates/tui/src/ui/theme/ansi256.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/TABLES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
