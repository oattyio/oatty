oatty-tui — Terminal UI

Overview
- Fast, schema-aware TUI for the Oatty CLI.
- Two primary modes:
  1) Default palette: a single-line command input with fuzzy autocomplete and inline help/errors.
  2) Command Browser modal: guided UI with searchable command list and inline help.

Key Features
- Palette (power mode):
  - Fuzzy autocomplete (commands, flags, enum values; provider-backed values for common fields).
  - Ghost text & suggestion popup; Tab accept; Up/Down cycle.
  - Inline validation & inline error message below the input.
  - Ctrl+H: opens Help for exact or top command suggestion.
  - Async execution: throbber (spinner) while processing; input clears on completion; results/logs update.
- Command Browser modal (Ctrl+F):
  - Left: Commands list (filters from palette token when opened).
  - Right: Inline Help (usage, arguments, options) for the selected command.
  - Enter: sends the composed command back to the palette and closes the modal (does not execute).
- Results viewing:
  - Arrays → rendered as tables with heuristic column selection.
  - Objects → key/value list; scalars → plain text.
  - Table modal includes styled footer hints and scrolling.
- Safety: secret-like fields masked in tables; Authorization redacted in logs (via util).

Execution
- Live requests: via `oatty-api` with auth using `HEROKU_API_KEY`.
- Errors: show inline with hints (auth/network/permissions), and also log.

Keybindings
- Global:
  - Ctrl+F: toggle Command Browser.
  - Ctrl+H: open Help (palette context uses exact or top suggestion).
  - Ctrl+C: quit.
- Palette:
  - Up/Down: cycle suggestions; Tab: accept; Enter: validate + execute.
- Browser:
  - Up/Down/Enter: select/apply; hints line shows `Ctrl+F close  Enter send to palette  Esc cancel`.
- Table modal:
  - Up/Down/PgUp/PgDn/Home/End: scroll; Esc close.

Providers (Value Suggestions)
- Asynchronous provider hook for flags/positionals:
  - Implement `ValueProvider` to return suggestion items for a given `command_key` and `field`.
  - Provider metadata now carries input bindings: `ValueProvider::Command { command_id, binds }`.
  - The TUI passes an `inputs` map with earlier positionals and known flag values to providers.
  - Example: `addons info <app> <addon>` binds provider `addons:list` at `/apps/{app}/addons` with `{ app ← app }` so addon suggestions are app-scoped.

Dev Notes
- Rendering split across:
  - `palette.rs`: power-mode input, suggestions, dimming, throbber, inline errors.
  - `ui.rs`: layout, browser/help/table modals, styling.
  - `tables.rs`: array→table renderer and KV/scalar fallbacks.
  - `preview.rs`: request previews and CLI preview string.
- Async execution:
  - Spawns a background thread and runs a Tokio runtime inside it to execute the HTTP request; communicates back via channel.
  - Spinner advances on ticks; input clears when finished.

Usage
```bash
cargo run -p oatty-cli              # opens TUI
DEBUG=1 cargo run -p oatty-cli      # enables extra debug
HEROKU_API_KEY=... cargo run -p oatty-cli
```

Troubleshooting
- “Unknown command …” — Use the `group sub` form (e.g., `apps info`); use Ctrl+H to see help.
- 401 Unauthorized — Set `HEROKU_API_KEY`.
- 403 Forbidden — Check team/app access and role membership.
- Network error — Check connectivity/proxy; `RUST_LOG=info` for more details.
### Provider-backed Suggestions

The palette integrates ValueProviders inferred in the registry:

- Registry generation embeds `provider: Option<ValueProvider>` directly on each `CommandFlag` and `PositionalArgument`.
- The shared engine `ProviderRegistry` now implements the ValueProvider trait, letting the palette resolve provider IDs (e.g., `apps:list`) and fetch live values via the same HTTP client logic (`fetch_json_array`).
- Results are cached in-memory with a short TTL and merged with enum values and history in the suggestion list.

### SuggestionEngine (Encapsulated Suggestions)

- The palette delegates suggestion building to `SuggestionEngine` (module: `ui/components/palette/suggest.rs`).
- Engine inputs: the `Registry`, a list of `ValueProvider`s, and the raw input string.
- Engine outputs: a list of `SuggestionItem`s and a `provider_loading` flag used to toggle the spinner at the end of the input.
- Engine responsibilities:
  - Resolve commands or suggest command names when unresolved.
  - Parse positionals and flags from the input.
  - Suggest flag values combining enums and provider results.
  - Suggest positional values via providers, with placeholder fallback.
  - Signal loading when a provider binding exists but no provider results are available yet.
- Palette keeps UI concerns (ghost text, popup state, cursor) and merges an end-of-line `--` hint when appropriate.
