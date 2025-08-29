heroku-tui — Terminal UI

Overview
- Fast, schema-aware TUI for the Heroku CLI.
- Two primary modes:
  1) Default palette: a single-line command input with fuzzy autocomplete and inline help/errors.
  2) Command Builder modal: guided UI with searchable command list, inputs, and live command preview.

Key Features
- Palette (power mode):
  - Fuzzy autocomplete (commands, flags, enum values; provider-backed values for common fields).
  - Ghost text & suggestion popup; Tab accept; Up/Down cycle.
  - Inline validation & inline error message below the input.
  - Ctrl+H: opens Help for exact or top command suggestion.
  - Async execution: throbber (spinner) while processing; input clears on completion; results/logs update.
- Command Builder modal (Ctrl+F):
  - Left: Commands list (filters from palette token when opened).
  - Middle: Inputs (positionals then flags) with enum cycling and bool toggles.
  - Right: Command preview (execution form: `group sub --flag value`).
  - Enter: applies the constructed command back to the palette and closes the modal.
- Results viewing:
  - Arrays → rendered as tables with heuristic column selection.
  - Objects → key/value list; scalars → plain text.
  - Table modal includes styled footer hints and scrolling.
- Safety: secret-like fields masked in tables; Authorization redacted in logs (via util).

Execution
- Live requests: via `heroku-api` with auth precedence `HEROKU_API_KEY` > `~/.netrc`.
- Errors: show inline with hints (auth/network/permissions), and also log.

Keybindings
- Global:
  - Ctrl+F: toggle Command Builder.
  - Ctrl+H: open Help (palette context uses exact or top suggestion).
  - Ctrl+C: quit.
- Palette:
  - Up/Down: cycle suggestions; Tab: accept; Enter: validate + execute.
- Builder:
  - Up/Down/Enter: select/apply; hints line shows `Ctrl+F close  Enter apply  Esc cancel`.
- Table modal:
  - Up/Down/PgUp/PgDn/Home/End: scroll; Esc close.

Providers (Value Suggestions)
- Asynchronous provider hook for flags/positionals:
  - Implement `ValueProvider` to return suggestion items for a given `command_key` and `field`.
  - Example: suggest app names for `apps info <app>` and `--app` values.

Dev Notes
- Rendering split across:
  - `palette.rs`: power-mode input, suggestions, dimming, throbber, inline errors.
  - `ui.rs`: layout, builder/help/table modals, styling.
  - `tables.rs`: array→table renderer and KV/scalar fallbacks.
  - `preview.rs`: request previews and CLI preview string.
- Async execution:
  - Spawns a background thread and runs a Tokio runtime inside it to execute the HTTP request; communicates back via channel.
  - Spinner advances on ticks; input clears when finished.

Usage
```bash
cargo run -p heroku-cli              # opens TUI
DEBUG=1 cargo run -p heroku-cli      # enables extra debug
HEROKU_API_KEY=... cargo run -p heroku-cli
```

Troubleshooting
- “Unknown command …” — Use the `group sub` form (e.g., `apps info`); use Ctrl+H to see help.
- 401 Unauthorized — Set `HEROKU_API_KEY` or configure `~/.netrc` for `api.heroku.com`.
- 403 Forbidden — Check team/app access and role membership.
- Network error — Check connectivity/proxy; `RUST_LOG=info` for more details.

