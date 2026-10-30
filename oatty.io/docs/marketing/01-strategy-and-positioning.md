# Strategy and positioning

Status: approved messaging direction; audience demand and differentiation remain under validation

Last reviewed: October 18, 2026

## Strategic focus

Lead with the engineer's ownership of the system:

> AI is changing how you build. System ownerhip is on you.

The product promise is **Understand and operate your stack with Oatty.** Give engineers a question to try on their own environment. Show how Oatty helps them check the settings and connections behind an application, make a change, and check the result. AI explains why this work keeps mattering as agents help write more code.

Oatty helps engineers and their agents set up services, inspect configuration, debug integrations, and verify changes across vendors. It should be useful before something breaks, too.

Making an API call gets someone started. Answering a question or completing a task across services shows why they would use Oatty. Demonstrate that on a named build with tested integrations.

## Category and promise

Working category: **a shared command environment for engineers and their agents**.

Campaign lead:

> AI is changing how you build. You still own the system.

Product promise:

> Understand and operate your stack with Oatty.

Explanation:

> Oatty brings API commands and MCP tools into one catalog for you and your agent. Check configuration across services, run the same commands in the terminal, and verify changes.

Adoption message:

> Free and open source. Built for engineers and their agents.

Explain the tool in this order:

1. Engineers still need to understand and take responsibility for what they build with an agent.
2. Start with a question about one service, integration, or change. Show the values behind the answer and say what couldn't be checked.
3. The agent uses Oatty through MCP. The engineer can inspect and run commands from the same catalog in the terminal.
4. Keep the catalog and useful checks for the next task. Oatty is free and open source.

The agent reasons about the task. Oatty provides the commands, results, and workflows. It doesn't automatically map or manage the whole environment. Follow [product truth and claims](03-product-truth-and-claims.md) when describing capabilities.

## User and job

### Primary user

A senior backend/product engineer, technical lead, or hands-on founder who:

- Uses a coding agent.
- Owns deployment, troubleshooting, or configuration changes.
- Depends on several managed services and possibly an internal API.
- Repeatedly gathers information or coordinates actions across those services.
- Can authorize access through supported authentication paths.

Start research with small teams that have no dedicated platform team. Don't assume company size predicts demand. Look for engineers who own these tasks and do them repeatedly. They are the likely champions; an engineering manager or technical founder may control the budget.

### Platform development segment

Also test Oatty with platform and data-platform engineers. They can explore vendor APIs, test an integration, and use what they learn to write clients, tests, infrastructure definitions, and orchestration code.

| Audience | Immediate job | Proof to show |
| --- | --- | --- |
| Engineer owning an existing application | Check configuration and complete a task across services | Show the values behind the answer and check the resulting state |
| Engineer building platform integrations | Explore unfamiliar APIs, test assumptions, and implement a design | Review the code and infrastructure, then check the deployed integration |

Use the same product promise and separate demonstrations for these jobs. Platform engineers are another group to test, not a replacement for the primary user.

### Underlying customer problem

An engineer owns a feature, but its behavior depends on hosting, identity, DNS, storage, and observability services. Checking one vendor's dashboard doesn't explain how the others are configured. The engineer needs to compare settings and act across those services.

### Immediate reasons to try Oatty

| Entry question | Familiar trigger | Useful first result |
| --- | --- | --- |
| How does this application connect to its infrastructure and external services? | Taking ownership, joining a project, or preparing an integration | Relevant resources, observed connections, supporting values, and unknowns |
| What changed across these vendors that could explain this failure? | Unexpected behavior around a deployment or configuration update | Relevant history and evidence supporting or rejecting possible explanations |
| What else depends on this configuration, and what should I check before changing it? | A planned domain, endpoint, connector, or credential update | Known dependencies within the inspected scope and checks before and after the change |

Begin with one service, integration, or change. The first demonstration should check configuration before a planned change. Use history when investigating what changed, and say which services were checked when assessing dependencies. Current settings alone can't answer either question completely.

### The journey

Oatty provides value throughout the life of an application, not only when something breaks:

| Understand your stack | Operate your stack |
| --- | --- |
| Find information across services | Set up resources and integrations |
| Inspect configuration and current state | Configure services |
| Investigate how services relate | Coordinate changes across vendors |
| Explain unexpected behavior | Diagnose and resolve failures |
| Check assumptions and outcomes | Preserve repeatable API workflows |

Test each example before advertising it. Saved workflows currently support HTTP commands; the agent can call downstream MCP tools separately.

### Demonstration focus

Show one integration from setup through a later change. A candidate example checks an application's domain in DNS, hosting, and the identity provider:

> Before changing this application's domain, use Oatty to inspect its hosting configuration, DNS records, and identity-provider callbacks. Identify the relevant environments, show the values connecting these services, and list the checks to perform before and after the change.

1. Set up or inspect the required resources.
2. Retrieve configuration and identify the relevant environment.
3. Explain relationships using observed values and identify uncertainty.
4. Propose and, in a controlled fixture, carry out a defined change.
5. Verify the resulting configuration with separate checks.
6. Preserve the HTTP-backed checks for the next environment or change.

An injected mismatch can show debugging. Test the APIs, permissions, and execution before publishing the example.

Make the finding clear. For example, the test environment could have a callback using a staging domain while hosting points to production. Ask the agent to show the retrieved settings, explain its conclusion, and list what still needs checking. This example hasn't established a tested integration or automatic diagnosis.

### Progression after activation

After the first useful task across vendors, help the user:

1. Inspect the request, response, and execution logs.
2. Correct a generated command definition or connection setting when needed.
3. Import another API or configure an MCP plugin.
4. Save tested HTTP operations as a workflow, or use what they learned to write code and tests.
5. Reuse the catalog and workflows through TUI, CLI, or MCP.

The catalog is still there for the next question. As engineers add services, they keep the commands for those services in one searchable place, available to the agent and directly in the terminal. Useful HTTP checks can become workflows they run again.

Show that return visit: use an existing command to check a later change instead of rebuilding access for the demonstration. Commands and credentials still need maintenance. The catalog retains commands, not an automatically updated map of production.

## Agent-assisted platform development

Supporting use case:

> Try the API, check the result, and use what you learn in your code and tests.

Use Oatty to find API operations, try them, inspect configuration, and check results. The agent and engineer then write and review the implementation in their development tools. Oatty doesn't generate clients, export infrastructure, or convert workflows into code itself.

The division of responsibility is:

| Layer | Responsibility |
| --- | --- |
| Engineer and connected agent | Define intent, interpret evidence, test assumptions, and review the implementation |
| Oatty MCP, CLI, and TUI | Discover and execute supported operations; inspect results; preserve tested HTTP-backed checks |
| Vendor management APIs | Configure and inspect the services performing the work |
| Reviewed code and CI/CD | Own deployment, integration tests, desired resource state, and recurring orchestration |
| Data-plane services | Move, process, and serve data through their dedicated runtimes |

A candidate data-platform demonstration could use ClickHouse Cloud/ClickPipes, Kafka Connect or Airbyte, and Grafana or Superset. Choose one ingestion path and test its APIs and authentication before publishing. Listing these vendors doesn't establish Oatty compatibility.

1. Discover the relevant operations and inspect configuration using scoped access.
2. Test a controlled interaction in staging and check the intended result.
3. Record resource relationships, required fields, tested assumptions, observed errors, and explicit outcome checks.
4. Have the agent author integration tests, a typed client where useful, and the appropriate infrastructure or orchestration definitions.
5. Review the artifacts, reconcile resources created during exploration, and deploy through the chosen owner.
6. Have another engineer verify the integration using the artifacts and documented checks.

Check configuration and data separately. A running connector doesn't prove that expected data arrived or that consumers query the intended dataset. Use targeted queries or integration tests to check the data. Dedicated ingestion and processing services handle bulk movement.

Write the design in the tool that will maintain it. Terraform/Pulumi definitions manage desired resource state; Airflow/Dagster workflows manage recurring orchestration. Once those tools own a resource, make routine changes through them. Keep using Oatty where it helps with inspection, experiments, and checks.

### An internal API example

An engineer needs to check an internal deployment service alongside hosting and DNS. The internal service has request examples but no published OpenAPI schema. Have the agent write a supported schema, validate and import it, inspect a generated command, and test a read operation against a known result.

Now the internal service's commands sit alongside the vendor commands in the catalog. Use them together to answer an existing entry question, such as what to check before changing a domain. On the next task, reuse those commands and check that they still work. Verify this example on a named build before publishing; schema generation alone doesn't prove correct behavior.

### Preserve understanding as well as operations

Keep the catalog, configuration references, assumptions, checks, and reviewed code that another engineer needs to understand the integration. Ask them to check the result without repeating the original investigation. Oatty doesn't currently package these into a portable integration bundle.

Measure project work separately from ongoing operations. A good handoff may reduce daily Oatty use. Find out whether engineers return for changes, checks, new integrations, or investigations.

## Problem statement

Engineers need to understand the services they own. Oatty needs to earn a place by helping with work they already do repeatedly. Hiring trends alone don't show that engineers want it.

The combination to test is:

- The agent can add, discover, execute, and help correct schema-derived commands.
- The developer can inspect and run the same persistent catalog directly.
- Successful sequences can become reusable workflow artifacts.
- The agent and engineer can use tested interactions to write code, tests, and infrastructure.

Test whether engineers prefer this combination, complete the work correctly, and come back. Those results haven't been established yet.

## Strategic choices

### Connect ownership to a useful discovery

Give the engineer a question to try on their own environment, then explain how Oatty provides access. Show setup and checks as well as debugging. The product should make sense without a prediction about the job market.

### Show the shared catalog

Pair an agent call with the matching command in the TUI. Show inputs, responses, and logs. Show the agent's explanation in its own client; Oatty doesn't display its reasoning or a relationship graph.

### Prove repair, do not imply magic

Show a reproducible failure, the exact command or connection change, and a successful retry. Make clear that the agent supplies the diagnosis and correction.

### Introduce workflows after first success

Save working HTTP commands as a workflow when the sequence is useful again. Check the result in the services involved; an HTTP success response isn't enough.

### Prefer local developer value over enterprise governance

Focus on the engineer doing the work. Oatty doesn't currently provide the identity, policy, history, and authorization controls needed to promise enterprise governance.

### Make free and open source prominent

An engineer can inspect the code, extend integrations, and try Oatty without buying software. That doesn't prove lower total cost or unique features.

## Secondary applications

Other examples to test:

- Preserve a repaired integration for later use.
- Run a repeatable task across two or more catalogs.
- Replace a small brittle API script with a reviewable workflow.
- Inspect an agent-triggered operation in the terminal.
- Use Oatty as a client for another MCP server.

Test procedures and limits for platform migrations, bulk changes, compliance collection, and credential rotation before demonstrating them.

## Non-goals for current positioning

Do not lead with:

- A universal replacement for vendor CLIs, workflow engines, or API clients.
- Natural-language reasoning built into Oatty.
- Enterprise governance, policy enforcement, or full agent observability.
- Token reduction as the main benefit.
- Automatic API drift repair.
- Automatic topology discovery or a continuously maintained relationship graph.
- A guarantee that every API, schema, operation, or authentication flow works.

## Adoption model

The current adoption sequence is:

1. **Recognition**: the engineer names a real service and a question or upcoming task they would use Oatty for.
2. **Connection**: they install Oatty and connect an MCP client.
3. **Onboarding**: they import a supported schema or configure an MCP plugin and complete a useful command.
4. **Activation**: they complete a verified useful task spanning at least two vendors. They check the answer against retrieved evidence or check an operation's result against external state.
5. **Control**: they inspect or repeat the operation in the TUI.
6. **Verification**: they check the evidence or an operation's external result. Evaluate diagnosis and recovery if something fails.
7. **Retention**: they return for another real task or rerun a useful check.
8. **Advocacy**: they share a workflow, integration pattern, or recommendation.

The first successful call measures onboarding. Saving a workflow is a later sign of value. Activation doesn't require something to break.

TUI use and workflow reuse can show value, but aren't required for every user.

## Current priorities

1. Test one integration from setup through a later change on a named release and controlled environment.
2. Test installation, authentication, and first-command completion independently of task success.
3. Observe engineers with relevant responsibilities setting up services, checking changes, and investigating failures across vendors.
4. Compare against the same agent using direct vendor MCPs or CLIs.
5. Check correctness and voluntary repeat use before making stronger claims.
