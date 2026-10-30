# Product truth and claims

Status: source-verified baseline; release behavior requires release-specific checks

Source baseline reviewed: October 13, 2026

Messaging reviewed: October 18, 2026

Use this document to check marketing claims. Source inspection shows what the inspected code implements. Test the advertised release before promising that behavior to users. Code alone doesn't prove reliability, an advantage over competitors, or demand.

## Current product baseline

| Capability | Current evidence | Claim state | Public treatment |
| --- | --- | --- | --- |
| Import OpenAPI 3.x documents into a persistent catalog | Registry import and validation pipeline | Verified in source | Say Oatty imports supported OpenAPI documents; publish tested compatibility and limitations |
| Accept schema contents through MCP without a temporary source file | MCP import request supports inline JSON/YAML input | Verified in current checkout | Verify the released binary/client pairing before documenting the exact procedure |
| Let an external agent search, inspect, and execute catalog commands | MCP server exposes discovery, inspection, and runners | Verified in source | Attribute reasoning to the connected agent |
| Execute configured downstream MCP tools through Oatty | MCP command dispatch invokes the plugin engine and downstream client | Verified in source | State plugin and transport prerequisites; do not infer that saved workflows support MCP steps |
| Use the catalog directly in a terminal interface | TUI exposes discovery, inputs, execution, results, logs, and library configuration | Verified in source and public docs | Describe the human and agent as using the same catalog, not necessarily an identical UI path |
| Patch generated command definitions and change catalog configuration | Catalog management tools and specifications | Verified in source | Describe explicit correction; do not claim automatic repair or source-schema rewriting |
| Define and run reusable API workflows | Workflow engine, specifications, TUI, CLI, and MCP tools; runner rejects MCP-backed steps | Verified in source | Describe tested HTTP-backed operations; qualify individual controls by execution path |
| Search before loading complete command details | Discovery and inspection tools | Verified in source | Describe the mechanism; do not publish token or context savings without measurement |
| Reference secrets, redact selected output paths, and skip TUI history values detected as sensitive | Secret resolution, redaction utilities, log persistence, and TUI history filtering | Qualified | Do not claim comprehensive redaction; MCP workflow run history currently persists its input map without applying a redaction helper |

Primary implementation references:

- [`crates/registry/src/openapi_import.rs`](../../../crates/registry/src/openapi_import.rs)
- [`crates/util/src/openapi_validation.rs`](../../../crates/util/src/openapi_validation.rs)
- [`crates/mcp/src/server/schemas.rs`](../../../crates/mcp/src/server/schemas.rs)
- [`crates/mcp/src/server/core.rs`](../../../crates/mcp/src/server/core.rs)
- [`crates/mcp/src/plugin/engine.rs`](../../../crates/mcp/src/plugin/engine.rs)
- [`crates/engine/src/executor/runner.rs`](../../../crates/engine/src/executor/runner.rs)
- [`crates/mcp/src/server/workflow/tools/execution.rs`](../../../crates/mcp/src/server/workflow/tools/execution.rs)
- [`crates/mcp/src/server/workflow/services/history.rs`](../../../crates/mcp/src/server/workflow/services/history.rs)
- [`specs/MCP_CATALOG_TOOLS.md`](../../../specs/MCP_CATALOG_TOOLS.md)
- [`specs/WORKFLOWS.md`](../../../specs/WORKFLOWS.md)
- [`specs/MCP_WORKFLOWS.md`](../../../specs/MCP_WORKFLOWS.md)
- [`specs/WORKFLOW_GAPS.md`](../../../specs/WORKFLOW_GAPS.md)

## Approved claims

Use these claims within the stated limits:

- Oatty turns supported OpenAPI operations into a persistent command catalog.
- A connected agent can search, inspect, and execute commands through Oatty's MCP tools.
- A developer can search, complete, run, and inspect commands in Oatty's TUI.
- A connected agent can execute tools from configured, available downstream MCP plugins through Oatty's command path.
- The connected agent can supply an authored schema to Oatty's validation, preview, and import path.
- Oatty can update generated command definitions and connection settings without rewriting the provider's original schema.
- Oatty workflows preserve reusable multi-step tasks using supported HTTP-backed commands.
- Oatty uses search-first discovery rather than exposing every imported API operation as a separate top-level tool.

## Claims that require qualification

### Campaign framing and first discovery

The approved lead and promise are in [messaging and voice](02-messaging-and-voice.md#message-hierarchy). They describe engineers' responsibility and what Oatty helps them do. Don't turn them into claims that AI caused a hiring shift or that engineers need Oatty to stay employable.

Invite the engineer to try a question about one service, integration, or change. Show a tested example, such as a deployment passing while the app still fails. Make clear that the agent investigates the cause; Oatty doesn't automatically diagnose it.

Ask the agent to show the values it read, explain its conclusions, and say what it couldn't check. The catalog and useful operations remain available for later tasks. Rerun them to check the environment again. A persistent catalog doesn't provide continuous monitoring, automatic synchronization, or a complete picture of current state.

### Understand and operate your stack

Support the promise with tested examples of setup, configuration, information gathering, debugging, verification, and reuse across vendors. Name the build and integrations. Don't imply every service or operation works. See [strategy and positioning](01-strategy-and-positioning.md).

The agent interprets results and explains relationships. Oatty doesn't automatically match resources across vendors, discover topology, maintain a relationship graph, or calculate blast radius. The TUI shows commands, responses, and logs, not the agent's full reasoning or a diagram of the system.

To explain what changed, use audit or deployment history, earlier snapshots, or another tested historical source. Current settings alone can't show a change or its cause. For dependency questions, say which services and values were checked. Known dependencies aren't necessarily all the dependencies.

### Workflow reuse

Saved workflows currently support HTTP commands. The agent can call downstream MCP tools separately, but the workflow runner rejects MCP-backed steps. An MCP client can run an Oatty API workflow; that doesn't mean the workflow can contain downstream MCP calls.

### Platform development and artifact handoff

Use API requests and results to inform code and tests. The agent and engineer write and review clients, tests, infrastructure definitions, and orchestration code in their development tools. Oatty doesn't generate, export, deploy, or convert those files itself.

A request and response show one interaction, not the full API contract. Test retries, idempotency, errors, and the intended result explicitly. Oatty doesn't provide complete contract capture, trace replay, or portable integration-bundle export.

Use Oatty to configure services, try API calls, and check results. Dedicated runtimes move and process the data. Small queries can help check an integration, but don't establish streaming, bulk-ingestion, or throughput capabilities. Label vendor examples as proposed until tested on a named release.

### Verification

Show engineers how to check a result during setup, debugging, and changes. Name the check and show the values behind it. A successful tool call or HTTP response doesn't prove the requested outcome, explain its cause, or show that every relevant service was checked.

The agent or engineer writes these checks. Oatty makes the operations available; it doesn't decide on its own that an integration is correct.

### API compatibility

Say **supported OpenAPI 3.x documents and operations**. List tested schema shapes, authentication methods, and exclusions in the relevant guide. A successful import doesn't prove every operation works.

For an internal API without a schema, an agent can write one from docs or request examples, then validate and import it. Test the generated commands before advertising the example. Don't imply automatic API discovery or claim that competing tools can't work with internal APIs.

### Safety

Name the control: validation, preview, redaction, client approval policy, method routing, a confirmation in a particular UI, or a workflow check. Don't turn one control into a guarantee that every operation is safe.

The current `run_safe_command` path accepts MCP-backed commands without enforcing downstream read-only annotations. Use verified provider permissions or another enforced restriction for a read-only demonstration. The tool name isn't enough.

### Human review

Oatty supports inspection and preview, but direct MCP execution doesn't require an Oatty approval token for every write. Claim enforced approval only for a path that implements and tests it.

### Repair

The agent or developer can diagnose a failure and change a command or connection setting. Oatty doesn't automatically detect drift or guarantee that the correction works.

### Large catalogs and context use

Search lets an agent find commands without exposing every imported operation as a separate top-level tool. Before claiming savings, measure total task tokens, startup schema size, result volume, tool calls, latency, and success.

### Local operation

Oatty runs locally, with catalogs and workflows under the user's control. Other tools run locally too. Local execution alone doesn't prove isolation, compliance, or enterprise readiness.

Free and open source means users can try, inspect, and extend Oatty. It doesn't prove lower total cost, easier setup, or better security.

### Authentication

Current documentation covers tokens and headers. Claim OAuth authorization-code, PKCE, token refresh, SSO, or credential brokering only when the specific path is implemented and tested.

### Secrets, logs, and history

Oatty provides secret references, shared redaction helpers, redacted log persistence, and a heuristic filter that skips TUI history values detected as sensitive. These controls don't cover every path. In the inspected checkout, MCP workflow run history saves the resolved input map without first applying a redaction helper. Don't put raw secrets in workflow inputs until that path is reviewed and corrected. Check display, error, result, copy, export, and storage paths before making a broader claim. Use test credentials during validation.

## Blocked claims

Do not publish the following without new evidence and an update to this file:

- Works with every API or provides full API coverage.
- Reduces coordination cost, context use, tokens, steps, code, or time by a stated percentage.
- Automatically detects or repairs API changes.
- Automatically discovers or maintains cross-vendor topology, resource identity, or dependency relationships.
- Displays the agent's complete reasoning or a production relationship graph in the TUI.
- Executes mixed HTTP/MCP steps in saved workflows.
- Natively generates or exports typed clients, integration tests, or infrastructure/orchestration definitions.
- Automatically converts exploratory workflows into production implementations or resolves their resource ownership.
- Provides complete contract capture, replay, or portable integration-bundle export.
- Replaces ingestion, streaming, or high-volume data-processing runtimes.
- Guarantees that an agent completed the requested external outcome.
- Requires human approval before every sensitive or destructive action.
- Provides comprehensive semantic classification of safe and destructive operations.
- Provides complete agent observability, durable correlated traces, replay, or automatic diagnosis.
- Replaces vendor CLIs, CI systems, workflow engines, API clients, or enterprise gateways.
- Is the first, only, best, safest, simplest, or fastest product in the category.
- Enforces read-only access to every downstream MCP tool through `run_safe_command`.
- Automatically understands or manages your entire production environment.

## Evidence needed to unlock stronger claims

| Claim area | Required evidence |
| --- | --- |
| First-call speed | Repeated tests on a named release, environment, API fixture, and participant cohort |
| Comparative setup or task performance | Same task, schema, credentials, agent/client, model, and output requirements across alternatives |
| Context or token efficiency | End-to-end token and tool-schema measurements, including native deferred discovery where supported |
| Reliability | Repeated runtime tests covering import, discovery, execution, restart, correction, and concurrency |
| Approval enforcement | Dispatch-level binding between an approved operation, arguments, target, catalog revision, and execution |
| Outcome verification | Explicit postcondition checked against external state, separate from HTTP or tool success |
| Observability | Correlated agent request, tool calls, nested operations, logs, results, duration, and durable export verified at runtime |
| User value | Observed task completion, voluntary repeat use, and evidence that Oatty displaced an existing workaround |
| Cross-vendor understanding | Correct resource and environment mapping, conclusions supported by retrieved values, and explicit handling of missing or conflicting evidence |
| Change investigation and dependency assessment | Appropriate historical sources for change claims, declared inspection scope, explicit uncertainty, and checks for the proposed explanation or change |
| Platform-development handoff | Reviewed artifacts deployed through the intended owner, independent outcome verification, and another engineer able to explain and reproduce the checks |

## Release verification checklist

Before publishing a campaign or guide for a release:

1. Record the Oatty version or commit.
2. Test installation from the advertised channel.
3. Connect the named MCP client using published instructions.
4. Validate, preview, and import the example schema.
5. Discover and execute the example operation through MCP.
6. Execute and inspect the same operation in the TUI.
7. Reproduce a failure and correction when repair or debugging is part of the advertised task.
8. Confirm authentication, secret references, redaction, history persistence, and cleanup behavior.
9. Update screenshots, recordings, commands, and limitations.
10. Store the test result and date with the campaign evidence.

Complete the advertised task across vendors and check the result separately. Show useful setup or configuration work without requiring a failure. For an example about understanding a service, show the finding, the values behind it, what couldn't be checked, and what to check next. When claiming reuse, return after a configuration change and rerun the checks.

For the copyable setup prompt, test installation or reuse of an existing install, preservation of client settings, local server startup, reconnection, and actual tool discovery. Record any manual steps. Check the first service read separately once access is available. A working Copy button doesn't verify the setup procedure or establish support for every agent client.
