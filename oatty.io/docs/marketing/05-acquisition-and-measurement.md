# Acquisition and measurement

Status: active plan

Last reviewed: October 18, 2026

## Acquisition objective

Reach engineers who use a coding agent and own work across several production services. Help them answer a real question about their environment, then see whether they return for another task.

The first successful API call is a setup milestone. Count activation when the engineer gets a checked answer or completes and verifies a useful task involving at least two vendors. Track later use for setup, configuration, investigation, debugging, verification, and reuse. Each task should include checks of the intended result.

Visits, downloads, and positive comments do not establish adoption.

## Website narrative

Use this sequence:

1. “AI is changing how you build. You still own the system.” Follow with “Understand and operate your stack with Oatty.”
2. A familiar experience: a deployment succeeds while the application fails, or a setting looks right in one dashboard but conflicts with another service. Also show routine work such as preparing a configuration change.
3. A question the reader can ask about their own environment, with a guide showing how to start.
4. The agent using supported APIs or MCP tools, and the engineer inspecting or running the same commands in the TUI.
5. An answer supported by the results, or a change checked in the affected services. Show what is known, inferred, and still unknown.
6. Configured catalogs and tested operations reused for later work, HTTP steps saved as a workflow, or findings used to write maintained code.
7. Installation and connection guides with tested versions, authentication instructions, and limitations.

Show useful work before, during, and after a change. Explain how Oatty helps engineers guide and check agent-assisted work. Avoid warnings about career relevance. The agent reasons about the results; Oatty provides the commands and access to the services.

The root README, homepage, and package description have matching local edits. Publication, released package metadata, and onboarding with a released build remain unverified. Before publishing, check the copy against [messaging and voice](02-messaging-and-voice.md) and [product truth and claims](03-product-truth-and-claims.md).

## Site requirements

- Serve complete homepage and documentation content in the initial HTML response.
- Allow one local clipboard script for the setup prompt. No other client-side JavaScript or third-party scripts.
- Use native HTML/CSS interactions and ordinary navigation.
- Keep metadata, canonical URLs, navigation, and contextual links consistent.
- Check the built site with JavaScript disabled: the complete prompt must remain readable and manually copyable. Verify that only the approved clipboard script is loaded; reject inline scripts, event handlers, and `javascript:` URLs.
- Preserve public routes when reorganizing content.

Keep pages complete in the initial response. The clipboard script is a convenience, not a page renderer or installer. Don't claim that this design improves search ranking.

## Acquisition content

Start with guides that answer these questions:

1. “How does this application connect to its infrastructure and external services?” Start with one service or integration and show the configuration values that connect it to others.
2. “What changed across these vendors that could explain this failure?” Use vendor history or saved snapshots. Current configuration cannot show what changed.
3. “What else depends on this configuration, and what should I check before changing it?” Show the dependencies found, the checks to run, and the parts the investigation did not cover.

Then cover:

- Setting up and checking an integration across services.
- Connecting an agent to APIs and MCP tools, then inspecting or running the same operations in the TUI.
- Writing a schema for an internal API from documentation or request examples, testing its commands, and using it alongside vendor APIs.
- Saving working API steps as a reusable workflow.
- Testing a platform integration and using the findings to write reviewed code and infrastructure definitions.

Articles about systems thinking and AI should lead to a task guide. Agreement with the premise does not mean someone will use Oatty.

Setup guides should cover OpenAPI import, writing a minimal schema when none is published, and fixing command definitions or connection settings. Avoid near-duplicate vendor or client pages. Each guide needs a tested procedure, expected result, limitations, recovery steps, and a useful next link.

Offer a setup prompt on the homepage and API-access guide, with a Copy prompt button and a manual-copy fallback. Keep the wording in sync with [the canonical prompt](../../public/setup-prompt.txt). Keep it to one line and 200 characters maximum. It should direct the assistant to install or reuse Oatty, connect the user's existing MCP client, and verify the connection. Test a service read separately. Explain any remaining TUI, permission, or reconnect step rather than promising setup will always finish from one prompt. Keep the manual instructions available.

Show how the configured catalog remains useful as engineers add services and repeat operations. Tested commands and saved connection settings can carry into later work. They still need maintenance; Oatty does not automatically keep a model of the environment current.

## Distribution

### Owned surfaces

- Homepage and public documentation.
- Root README and repository About metadata.
- Package description and package homepage.
- Demonstration video and technical write-ups with supporting evidence.
- Release notes explaining what an engineer can now do.

### Targeted outreach

Share examples where developers discuss the task: integration setup, configuration changes, finding information across vendors, production investigation, or verification. Show the work and the result.

For platform engineers, publish tested examples with agent-written, engineer-reviewed code that others can adapt. Name the maintainer, supported versions, test environment, and limitations. Keep credentials out of published files.

### Search

Build pages around real tasks and questions. Titles and links should describe what the guide helps someone do. Treat search terms as guesses until Search Console provides impressions and queries.

### Community proof

Ask early users for reproducible feedback, integrations, workflows, and failure reports. Obtain permission before publishing private praise, and include its context.

## Demonstration standard

The main demonstration should show:

1. The application, participating services, and a specific question about the engineer's environment.
2. Connection and catalog setup using tested APIs or MCP tools.
3. Agent calls across the services.
4. The same commands and their results in the TUI.
5. The evidence behind the answer, or a separate check that the change worked. Include unknowns and next checks.
6. A way to repeat the checks, saving HTTP-backed steps as an Oatty workflow where useful.

For example, before changing an application domain, inspect DNS records, the hosting domain binding, and identity-provider callbacks. Show the environments and exact values connecting the services. Identify settings that agree or conflict, note dependencies that could not be checked, and list checks before and after the change. Setup and debugging can extend the example; a failure is not required.

Saved Oatty workflows execute HTTP-backed steps only. Direct MCP calls do not establish mixed HTTP/MCP workflow support.

Include an internal API without a published schema. Have the agent write a minimal schema from its documentation or request examples, import it, and inspect and test the resulting command. Use it alongside a vendor API to answer one of the questions above. Return to that catalog for a later task and show what can be reused and what needs updating.

To investigate what changed, include audit events, deployment records, or dated snapshots. State when history is unavailable. A configuration difference alone does not establish a timeline or cause of failure. Measure time to a useful discovery before making a speed claim.

A controlled test environment is acceptable. Disclose synthetic data and injected errors. Record the Oatty commit or release and MCP client. Keep machine-readable results when practical. The TUI does not automatically reconstruct the agent's reasoning or service relationships.

### Platform development demonstration

Follow the [platform development use case](01-strategy-and-positioning.md#agent-assisted-platform-development) with two or three vendor APIs. Explore their operations, test how their settings relate, and check that the integration works. Then show the agent and engineer writing the appropriate typed client, integration tests, infrastructure definitions, or orchestration code. The example does not need every artifact type.

Use Git review and CI/CD. Have a second engineer deploy the reviewed code and verify the result without repeating the exploration. Ask them to explain the service relationships, evidence, and checks.

Oatty supports the exploration; the agent and engineer write the code. Terraform/Pulumi definitions describe desired state. Airflow/Dagster runs recurring orchestration. External services move bulk data. For ingestion, check that a small amount of expected data arrived; a successful configuration call is insufficient. Decide which code or tool owns each resource, and reconcile experimental changes before handoff.

## Funnel

| Stage | User evidence | Primary measure |
| --- | --- | --- |
| Discovery | An engineer with relevant work reaches the content | Relevant visits by source and query |
| Intent | The visitor chooses a question and starts its guide or installation | Question selection, guide entry, and primary-action use |
| Connection | The MCP client connects to Oatty | Verified connection |
| Onboarding | One useful operation succeeds | First successful agent call with a checked result |
| Activation | A useful task involving at least two vendors is verified | First answer supported by evidence or result checked in the affected services |
| Direct use | The engineer inspects or repeats commands in the TUI | Direct use of the same catalog commands |
| Retention | The engineer returns for another useful task | Repeat use within a defined period |
| Reuse | The engineer saves or reruns HTTP-backed steps as a workflow | Workflow reuse after activation |

TUI use and saved workflows are useful signals, not required steps. Record diagnosis and recovery when a failure occurs.

Keep analytics out of the clipboard script. Product, server, repository, package, and Search Console data can be assessed separately.

## Search baseline

The October 10 public audit found:

- No obvious robots or homepage indexing exclusion.
- All 16 sitemap locations returned successfully after redirects.
- Public search samples showed some branded retrieval.
- Search Console and authenticated analytics were unavailable.
- The old JavaScript-rendered homepage gave non-rendering clients a thin initial explanation.
- Public repository metadata and content distribution were weak.

The site now serves complete static HTML, and the local copy uses the systems-thinking and ownership message. Publication and any improvement in search discovery remain unverified.

Keep the dated [search evidence snapshot](evidence/search-discoverability-2026-10-10.json) as the baseline before the rewrite.

## Measurement plan

### Search and discovery

- Search Console page indexing and sitemap status.
- URL Inspection for the homepage and main guides.
- Branded and non-branded query impressions, positions, and clicks.
- Referrals from GitHub, package pages, demonstrations, and community posts.

### Onboarding and activation

- Installation success for the advertised channel.
- MCP connection success.
- Schema validation and import success.
- Time and help needed for the first useful call.
- Time and help needed for the first verified task involving at least two vendors.
- Evidence supporting the answer, or a separate check that the change worked.
- Task type and failure reason at each step.
- Whether the engineer can name a real service and useful question after reading the message, and then try it.
- Whether the first discovery answers that question with evidence and clear limits.

### Retention and value

- Return for a second useful task or session.
- Reuse of an integration, verification check, or investigation procedure.
- Reuse of configured catalogs and tested commands as services and tasks are added, including maintenance needed to keep them correct.
- Direct TUI use after agent execution.
- Workflow creation and rerun after activation.
- Replacement of a script, wrapper, or other workaround.
- Independent deployment and verification of reviewed platform code.
- Completed platform projects versus continued use for inspection, changes, verification, or new integrations.
- Returns to check assumptions or verify agent-assisted changes as the environment evolves.

Before setting numeric targets, define each event, how it is collected, its privacy treatment, its owner, and the baseline. Earlier goals of a ten-minute first workflow and 50 percent workflow conversion were unmeasured and are not current commitments.

## Interpretation rules

- Few indexed pages suggest a crawl, indexing, or site-structure issue.
- Indexed pages with few impressions suggest weak relevance, demand, or authority.
- Impressions without clicks suggest a mismatch in query, position, or search-result copy.
- Relevant visitors who do not connect may face setup or trust concerns.
- Connections without a useful call suggest schema, authentication, discovery, or runtime issues.
- Successful calls without a verified task across vendors suggest the advertised value is still out of reach.
- Verified tasks without return use may indicate an infrequent need, little advantage, or sufficient alternatives. A completed platform handoff can still be valuable; record it separately and test what brings those engineers back.

At low traffic, use moderated trials and targeted distribution. Run traffic experiments when there are enough visitors to draw useful conclusions.

## Operating cadence

Review monthly:

1. Sources bringing relevant engineers.
2. Failures before the first verified task.
3. Repeat-use evidence.
4. Search Console changes.
5. New competitor behavior.
6. Claims to strengthen, qualify, or remove.

Put findings in the document covering that decision. Create a separate assessment only when it is a dated evidence record with a plan to fold its conclusions into the main docs.
