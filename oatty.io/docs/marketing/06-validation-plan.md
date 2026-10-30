# Validation plan

Status: active research protocol

Last reviewed: October 18, 2026

## Purpose

Find out whether Oatty helps engineers understand and operate their stack, and whether they choose to use it again.

Test whether the message in [messaging and voice](02-messaging-and-voice.md) helps an engineer recognize useful work they could do in their own environment. Agreement about AI or systems thinking is insufficient. Look for a real question, a completed task, a checked result, and a reason to return.

Keep three kinds of evidence separate:

1. Capability: what a named build actually does under stated conditions.
2. Demand: what engineers struggle with and what their current workaround costs.
3. Comparative value: whether Oatty helps with the same task more than the alternatives.

## Decisions this research must support

| Decision | Question |
| --- | --- |
| Audience | Which engineers regularly need to work across vendors? |
| Positioning | Do engineers understand the message and name a task they would try in their own environment? |
| Product | Which connection, configuration, inspection, verification, recovery, and reuse capabilities do they need? |
| Differentiation | Does the shared catalog help when clients already offer discovery and logs? Does it remain useful as engineers add services and repeat operations? |
| Distribution | Which channels bring engineers who try the tool for relevant work? |
| Business model | Who keeps getting value, and what maintained service might they pay for? |

## Capability audit

Check claims against a named commit or release. Record:

```text
Capability:
Documentation claim:
Observed behavior:
Version or commit:
Environment and client:
Reproduction procedure:
Limitations:
Evidence:
Confidence:
```

Check:

- OpenAPI input, validation, import, and unsupported schema shapes.
- Catalog discovery and MCP tools.
- Shared TUI, CLI, workflow, and MCP execution, including the HTTP-only saved workflow limit.
- Calls across vendors and separate checks that the intended result occurred.
- Which relationships are explicit in configuration, which the agent inferred, and which dependencies it could not check.
- Access to vendor history or dated snapshots for questions about changes. Current configuration alone is insufficient.
- Authentication, secret storage, redaction, and authorization.
- Preview, approval, cancellation, retry, recovery, and interruption.
- Logs, correlation, persistence, export, and behavior after restart.
- Installation and time to the first useful result.
- Reuse of configured catalogs and tested operations across tasks and services, including the work needed to keep them correct.

Telling an agent to ask permission does not prove that the execution engine enforces approval. A configured catalog is reusable access to operations, not an automatically updated model of the environment.

## Customer research

Recruit 10 engineers across several teams. This is a discovery sample, not a representative market survey. Each should:

- Already use a coding agent.
- Own deployment, troubleshooting, or configuration changes.
- Depend on several managed services and possibly an internal API.
- Have recently worked across vendors using dashboards, commands, or colleagues.
- Be able to authorize the required API access.

Include some platform and data-platform engineers to test the [platform development use case](01-strategy-and-positioning.md#agent-assisted-platform-development). Keep the main focus on engineers responsible for production systems.

Before showing Oatty, ask about their most recent task across vendors. Include setup, configuration, information gathering, investigation, debugging, and verification. The task need not involve a failure:

- What triggered it?
- Which systems did they inspect?
- What was confusing?
- How long did it take?
- Does it recur?
- What do they maintain now?

Then test the message and the product:

1. Show the proposed copy and explain supported API/MCP access and direct command inspection.
2. Ask what the product does, what experience the copy recalls, and how it makes them feel about trying it. Record confidence, pressure, or confusion without suggesting the answer. Enthusiasm for AI is not a recruitment criterion.
3. Ask them to name a service, integration, or upcoming change and a specific question they would try to answer.
4. Observe their normal process for that task, or a comparable test case involving at least two vendors.
5. Test the same agent with direct vendor MCPs or CLIs.
6. Test the same agent using Oatty.

If they need examples, use the three existing questions: how an application connects to services, what changed around unexpected behavior, and what to check before a change. Record when examples are needed. Include routine setup and configuration work.

Keep the task, model, permissions, and required answer or result the same. Vary the order of the tools across participants, or use equivalent test cases, to limit learning effects. Measure setup separately from task execution. Record:

- Correct relationships found.
- Unsupported conclusions.
- Time to an answer supported by evidence, or a result checked in the affected services.
- Whether requested settings or changes are correct.
- Whether another engineer can repeat the task or checks.
- Whether the participant can explain the relationships, evidence, unknowns, and result without repeating the agent's conclusion.
- Whether the first discovery answers their question and informs a decision or next action.
- Whether checks cover the actual goal during setup, configuration, debugging, and changes, rather than only HTTP or tool success.

Record completion, help needed, errors, later use, and the alternative they would otherwise choose. Positive comments alone do not show adoption.

## Comparative benchmark

Compare Oatty with:

- The same agent using direct vendor MCPs or CLIs.
- The same agent using Bruno's local collections and MCP server, including its tests and environment support.
- A small hand-written client or the developer's normal API tool.
- A credible OpenAPI-to-MCP framework or generator.
- An API-aware CLI where it can perform the task.
- Native deferred discovery or code mode when the chosen agent client supports it.

Keep these the same:

- Vendors, API/tool coverage, and schemas.
- Credentials and environment.
- Agent client, model, instructions, and context.
- Task and success criteria.
- Required outputs and checks.
- Whether setup starts fresh or reuses an existing configuration.

Measure:

- Setup time and help needed.
- Successful completion across repeated runs.
- Number and type of corrections.
- Tool calls, tokens, latency, and output volume.
- Diagnosis and recovery time.
- How well the engineer can inspect the operations and results.
- Persistence and reuse after restart.
- What catalog configuration and tested operations remain useful for the next task, and what needs updating.

Report Oatty's losses and regressions. Include competitors' existing capabilities and use a fair baseline. Keep setup comparisons out of marketing.

## Required scenarios

### First API call (onboarding milestone)

Import a supported schema, find one operation, execute it through MCP, and check the expected result.

### Agent-authored schema

Use an internal API with usable documentation or request examples but no published OpenAPI document. Ask the agent to write a minimal schema, validate and import it, inspect the resulting command, and complete a defined read. Check the response against the documented behavior.

Then use that API alongside a vendor API in an existing environment, investigation, or change question. Restart and reuse the catalog for another operation. Record what the saved configuration makes reusable, what needs correction, and whether the engineer can inspect and run the same command. Oatty supplies access; the agent establishes relationships from the results.

### Repair

Introduce a documented schema or connection defect. Record the failure, proposed correction, catalog change, successful retry, and remaining unknowns.

### First verified cross-vendor task (activation event)

Complete a useful task involving at least two vendors. Count activation only when the answer has supporting evidence or the intended result is checked in the affected services. An HTTP success or successful tool call is insufficient. Use the comparison and scoring above.

Cover:

- Setting up an integration and checking its settings across services.
- Gathering information or comparing configuration before a change.
- Making a configuration change and checking that it had the intended effect.
- Investigating or debugging unexpected behavior across vendors.

Use appropriate test environments and permissions. Record which tasks bring engineers back. Do not require an injected failure.

For one test case, inspect DNS records, the hosting domain binding, and identity-provider callbacks before a domain change. Identify environments and exact values connecting the services. List conflicts, dependencies that could not be checked, and checks before and after the change. Ask what the engineer learned and how it affects the decision. Do not assume Oatty builds a complete dependency graph.

For change investigations, provide vendor history, deployment records, or dated snapshots. Count unsupported claims about changes or causes as errors. When history is unavailable, the answer should say so and identify the next evidence needed. Keep current-state findings separate from historical evidence. Measure how quickly a useful discovery occurs; do not promise a speed that has not been measured.

### Platform integration and maintained handoff

Use two or three services. Explore their APIs, test how their settings relate, and check that the integration works. Ask the agent and engineer to write the appropriate typed client, integration tests, Terraform/Pulumi definitions, or Airflow/Dagster code. Record these as code they wrote using their findings, not native Oatty generation or export.

Review through Git and CI/CD. Have a second engineer deploy the reviewed code in a controlled environment and check the result without repeating the exploration. Score correct relationships, tested assumptions, error and retry behavior, checks, and the handoff. For ingestion, check that a small amount of expected data arrived as well as checking configuration. External services move bulk data.

Terraform/Pulumi definitions must describe desired state and establish which tool owns each resource. Reconcile experimental writes before handoff. Name the runtime responsible for recurring orchestration. A saved sequence of calls is not an infrastructure definition. Use only the artifact types the example needs. Saved Oatty workflows execute HTTP-backed steps only.

After handoff, record whether engineers return for inspection, verification, changes, or another integration. A completed project can be valuable without continued Oatty use. Test what configured catalogs and tested operations they reuse; do not assume the catalog tracks later changes automatically.

### Shared human execution

Find and run the same operation in the TUI. Ask the participant to trace an agent's conclusion to the supporting calls and results. Determine whether this improves understanding, confidence, or recovery. Distinguish confidence based on checked evidence from confidence in a persuasive answer.

### Reuse

Restart and repeat the task. Add another service or perform another related operation using the configured catalogs. Record what carries over, what must change, and whether engineers choose this access path again.

Save a multi-step workflow only after the individual HTTP calls work reliably. Test repeat use of direct MCP calls separately; saved workflows do not execute downstream MCP steps. Repeat the checks of the intended outcome, not just the calls.

### Authentication and redaction

Use test credentials. Check that configuration, storage, logs, results, errors, and exported evidence do not expose raw secrets.

## Demand evidence

Look for descriptions of work, not just product categories. Search public issues, discussions, forums, and interviews about:

- Setting up integrations or coordinating configuration across vendors.
- Finding information and reconstructing service relationships.
- Taking ownership of an unfamiliar environment or preparing a configuration change.
- Successful deployments followed by application failures, and conflicting settings across dashboards.
- Checking a change using evidence from several systems.
- Giving an agent access to an internal API.
- Repairing a generated tool or schema.
- Too many tools or difficulty finding the right one.
- Inspecting actual requests and responses.
- Agents claiming success when the intended result did not occur.
- Reusing an agent-created integration or task.
- Keeping useful API access and tested operations available for later work.
- Using API experiments to write maintained code, infrastructure definitions, and tests.

Record each item as:

```text
Exact excerpt, permalink, and date:
Author role, when stated:
Problem and trigger:
Current workaround:
Consequence:
Requested capability:
Evidence type:
Independence from other samples:
Strength and limitations:
```

Look for counterexamples too: engineers satisfied with client logs, scripts, API clients, unattended execution, managed connectors, or browser/IDE tools. Record why these suffice and what would stop them adopting a local terminal tool.

Promotional copy, duplicate posts, stars, and downloads do not independently establish demand. Counts describe the collected sample, not how common a problem is across the market.

## Reporting discipline

Use this structure:

1. Finding.
2. Evidence.
3. Interpretation.
4. Product or marketing implication.
5. Uncertainty and next test.

Distinguish documentation, vendor claims, observed behavior, user reports, and researcher inference. Date competitor and traffic evidence.

## Decision criteria

Continue with the positioning when several participants:

- Recognize recurring work without extensive explanation.
- Name a question about their own environment and start a relevant trial.
- Complete and verify a task across vendors with limited help.
- Prefer Oatty for that task over their workaround or direct agent tools.
- Return for another useful task.
- Reuse or share the configured catalogs, integration, checks, or workflow.
- For platform work, explain the integration and hand over reviewed code that another engineer can deploy and verify.

Reconsider when participants consistently:

- Prefer a script, generated server, or managed connector.
- Need substantial support to configure authentication.
- Find little value in the shared catalog, direct inspection, or reusable operations.
- Treat integration setup and maintenance as disposable work.
- Agree with the AI or systems-thinking premise but cannot name useful work or take a next step.
- Complete one task but do not return for recurring work. Assess completed platform projects and their handoffs separately.

Voluntary second use is the main signal for the free, open-source tool. The proposed early threshold is a majority returning for another real task within two weeks. This is a research decision rule, not an established benchmark.

If a maintained paid offering is defined, assess buying behavior separately. Acceptance of a specific paid pilot supports that offering, not adoption of the free tool by itself.

Report message recognition, comprehension, task choice, checked completion, and return use separately. A lasting role for Oatty needs evidence that engineers return to inspect state, guide changes, test assumptions, or verify results. Message preference alone does not validate the strategy.

## Research backlog

1. Establish Search Console and acquisition baselines.
2. Test the published installation and API-access guide end to end.
3. Run moderated setup and tasks across vendors, including recovery where relevant.
4. Compare current alternatives on the same task.
5. Test reuse after restart and transfer to another engineer, including independent deployment of platform code.
6. Find the most difficult authentication cases.
7. Revisit pricing or enterprise expansion after repeat-use evidence exists.
