Title: Feasibility of Moving TUI Palette Command Execution to Engine CommandRunner

Context
- Current path: crates/tui/src/cmd.rs::start_palette_execution parses the palette input, validates it against the registry, builds request body/args, persists some UI state, and dispatches a UI command (Cmd::ExecuteHttp or Cmd::ExecuteMcp).
- Engine runner: crates/engine/src/executor/runner.rs defines CommandRunner and a RegistryCommandRunner that can resolve a run identifier (group:cmd) to a CommandSpec and execute it via HTTP using HerokuClient. It already builds paths, translates with/body to query/body, and handles range header.

Findings
1) Functional overlap and gaps
- Overlap: RegistryCommandRunner already handles many responsibilities currently duplicated in TUI:
  - Converts positional args (when provided in `with`) into path variables and builds the final HTTP path.
  - Builds Range header from body and strips body fields.
  - Executes the request and returns a structured JSON result.
- Gaps: start_palette_execution covers parsing shell-like input into tokens, validating required args/flags (semantics driven by registry CommandSpec), and building a request body from flags. These are not yet encapsulated in the engine.
- MCP handling: RegistryCommandRunner explicitly errors on MCP commands. TUI currently supports MCP via Cmd::ExecuteMcp and a separate code path. Any move must preserve this branch in TUI or extend the engine with an MCP-aware runner.

2) Coupling to TUI
- The following are UI concerns and should not be moved:
  - Logging of the raw palette input to UI logs.
  - Palette history management and pagination state seeding (initial_range, pagination_history, last_* fields used by UI).
  - Emission of UI-specific Cmd variants.

3) Layers and dependencies
- Engine already depends on registry and util crates used by TUI (e.g., build_path, range header helpers).
- The shell-like tokenizer used in TUI (heroku_util::lex_shell_like) is available to engine as well; importing it would not violate layering.
- RegistryCommandRunner owns a reqwest client via HerokuClient; it creates a Tokio runtime internally for sync run(), which is acceptable in non-async callers but should be noted if the TUI uses async in the future to avoid nested runtimes.

4) Feasible refactor boundaries
- Minimal, low-risk move:
  - Introduce an engine helper (e.g., engine::executor::parse_palette_input or CommandPlan::from_input) that accepts:
    - input: &str
    - registry: &Registry
  - Returns a ParsedCommand/CommandPlan struct containing:
    - spec: CommandSpec
    - run_id: String ("group:cmd")
    - with: serde_json::Value (Object) — combined flags and positional args by name
    - body: Option<serde_json::Value> — flags interpreted as body where applicable
    - pos_args: Vec<String> (optional, for UI persistence)
  - This moves parsing/validation/construction out of TUI while keeping UI state management and command dispatch in TUI.
- Optional extension:
  - Add a convenience method to RegistryCommandRunner: run_input(&self, input: &str) -> Result<Value> that uses the helper above + internal registry, delegating execution to existing run().
  - Keep MCP as a distinct code path (either: return a structured error indicating MCP so the caller can route to MCP executor, or implement a separate McpRunner in engine and a composite runner that dispatches by CommandExecution).

5) Risks and considerations
- MCP split-brain: Since RegistryCommandRunner errors on MCP, the TUI must still branch for MCP. Adding an engine-level dispatcher that supports both HTTP and MCP would be a larger change.
- Pagination features in TUI depend on the request body/flags; ensure the returned plan exposes the range-related fields so TUI can seed pagination UI without re-parsing.
- Runtime creation inside runner: constructing a Tokio runtime per call is convenient but may be inefficient; however, this is pre-existing and not introduced by the move.

Conclusion
- Moving the "command execution path" responsibilities from start_palette_execution to the engine is feasible if scoped to:
  - Parsing + validation + construction of a CommandPlan in the engine, and
  - Optionally, execution via RegistryCommandRunner for HTTP commands only.
- UI responsibilities (logging/history/pagination) should remain in the TUI.
- MCP execution should remain in the TUI for now (or be addressed by a future engine runner addition).

Recommended incremental plan
1. Add CommandPlan/ParsedCommand type and parse_input(input, &Registry) -> Result<CommandPlan> to engine (no TUI deps). Expose via engine::executor.
2. Refactor TUI start_palette_execution to call parse_input, update UI state based on the returned plan, and then:
   - If plan.execution == HTTP: either dispatch to existing TUI HTTP executor as today (no behavior change), or call RegistryCommandRunner::run(plan.run_id, Some(plan.with), plan.body.as_ref(), &ctx) and render results.
   - If plan.execution == MCP: route to existing MCP executor.
3. Keep TUI pagination/history logic unchanged; use the CommandPlan view of range fields to seed UI.
4. Optionally, add RegistryCommandRunner::run_input(&self, input: &str) -> Result<Value> for CLI-like integration outside the TUI.

Scope estimate
- Engine additions: ~150–250 LOC (types + parser + validation + public API).
- TUI refactor: ~60–120 LOC modifications in cmd.rs to delegate to engine and adapt return types.
- No breaking changes to public CLI or UI behavior expected.
