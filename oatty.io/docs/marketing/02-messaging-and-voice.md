# Messaging and voice

Status: current

Last reviewed: October 18, 2026

## Message hierarchy

Use the lead to get attention, then explain what Oatty does and give the engineer a question to try. Short descriptions can start with the product explanation.

### Campaign lead

> AI is changing how you build. You still own the system.

### Product promise

> Understand and operate your stack with Oatty.

### Familiar experience

> The deployment passed, but the app still fails. The hosting domain changed, but the login callback didn't. Finding the cause means checking more than one service.

This is one example. Also show setup and planned changes, so Oatty doesn't sound useful only when something breaks.

### Explanation

> Oatty brings API commands and MCP tools into one catalog for you and your agent. Check configuration across services, run the same commands in the terminal, and verify changes.

### Immediate invitation

Primary action: **Explore your environment.** Link to the connection guide and put a starting question nearby:

- What does this service connect to?
- What changed around this failure?
- What should I check before changing this configuration?

Start with one service, integration, or change. For a change investigation, use deployment history, audit events, or earlier snapshots. For a dependency question, say which services were checked and what is still unknown.

### Adoption message

> Free and open source. Built for engineers and their agents.

### Mechanism

> Import an OpenAPI schema or connect an MCP server. Your agent can find and run commands through Oatty. You can use the same catalog in the terminal.

Use supported schemas and configured MCP plugins in examples. [Strategy and positioning](01-strategy-and-positioning.md) defines the audience and promise; [product truth and claims](03-product-truth-and-claims.md) defines the claim limits.

### Continuing relevance

> Keep the commands that worked. Use them when you add a service, change a setting, or need to check the result.

The catalog stays available for later tasks. Rerun commands to check configuration; a saved catalog is not a saved or automatically updated model of the environment.

### Proof

Show a real task from setup through checking the result:

1. Connect the MCP client.
2. Import the relevant API schemas or configure MCP plugins.
3. Answer a question or complete a task involving at least two vendors.
4. Show the agent's explanation and the values it used in the agent client. Inspect or repeat the relevant commands in the TUI.
5. Check the answer or resulting state against the services involved. A controlled failure can show debugging.
6. Save useful HTTP operations as a workflow and run it again on a later task.

### Expansion

After the first useful task, show another question the same catalog helps answer. Add other APIs, CLI use, and workflows as needed. Test every demonstration on the named build and integrations.

## Supporting messages

### Work across the entire application lifecycle

Set up services, check configuration, debug integrations, and verify changes. The agent reasons about the task; Oatty gives it the commands to work with.

### Investigate configuration across vendors

Compare the settings that need to agree. For a domain change, that might mean a DNS record, a hosting domain, and an authentication callback.

### Understand how services fit together

Ask your agent to show the values behind its explanation. Which settings match? Which conflict? What couldn't it check?

### Check that the change worked

Read back the configuration after a change and test the behavior that matters. An updated login callback is one check; a working login is another. Have the agent show the results and say which checks it couldn't complete. A successful API response alone doesn't prove the integration works.

### Use the same commands as your agent

Find a command, fill in its inputs, run it, and inspect the response and logs in the TUI. The TUI shows operations and results; the agent's explanation stays in its own client.

### Let your agent prepare and correct API access

Your agent can validate and import schemas, change connection settings, and correct generated commands. Inspect the changes and retry the operation. Test installation and client connection on the advertised release before promising setup from a single prompt.

For an internal API with no published schema, have the agent write a supported OpenAPI document from existing docs or request examples. Validate it, inspect a generated command, and test the result. Keep that catalog alongside vendor commands for later work. This is also useful for private or imperfect vendor APIs; don't claim there are no alternatives.

### Keep a task you will use again

As you add services, you keep their commands in one searchable catalog. Your agent can use it, and you can use it directly. You don't have to rebuild the catalog for each question, though credentials and commands may still need maintenance.

Save tested HTTP operations as a workflow when the sequence is worth repeating. Saved workflows do not currently support downstream MCP steps; the agent can call those tools separately.

### Carry what works into maintained code

> Try the API, check the result, and use what you learn in your code and tests.

For platform and data-platform engineers, show the agent using tested API interactions to write clients, integration tests, infrastructure definitions, or orchestration code. The engineer reviews those files in the usual development tools. Oatty does not generate or export them itself.

Follow the candidate example in [agent-assisted platform development](01-strategy-and-positioning.md#agent-assisted-platform-development). Choose a demonstration for operating an application or building an integration; one example need not cover both.

## Practical answers

### Does the API need to publish an OpenAPI document?

No. Your agent can write a supported document from API documentation or request examples. Validate the document and test the generated commands.

### Does Oatty contain its own language model?

No. You connect your own agent. Oatty provides the catalog, command execution, results, and workflows.

### What changes when a command is repaired?

Oatty changes the generated command definition. It doesn't rewrite the provider's OpenAPI document.

### Can a developer use Oatty directly?

Yes. Use the TUI to find and run commands, inspect results and logs, configure catalogs, and work with workflows.

### Does implementation have to remain an Oatty workflow?

No. Use what you learn to write code, tests, and infrastructure in your chosen tools. Keep Oatty checks where they help. Oatty doesn't automatically convert a workflow to Terraform, Pulumi, Airflow, or Dagster.

### Which authentication methods work?

Name the methods tested for the release and integration. Configuring a token or header doesn't establish support for OAuth login or token refresh.

## Voice

**Could a competent engineer say this to a colleague, in the same words, without wincing? If not, rewrite it.**

Oatty should sound like an engineer explaining a useful tool: clear about what it does, interested in the work, and honest about its limits. The reader should be able to picture using it.

- Say what someone can do and what they'll see. Name the services, settings, commands, or results when they matter.
- Use ordinary words and natural contractions. Keep technical terms when they make the explanation more exact.
- Make one point at a time. Cut repeated explanations of the strategy and long strings of abstract nouns.
- Be confident about supported behavior. Put a limitation beside the claim it limits, in plain language.
- Treat questions and uncertainty as normal engineering work. Don't blame the reader or imply they need Oatty to stay employable.
- Don't make every heading a slogan. A short label or a direct question often works better.

### Tone by situation

| Situation | Tone | Example |
| --- | --- | --- |
| Homepage or introduction | Direct and inviting | "Check configuration across services." |
| Setup or how-to | Specific and practical | "Import the schema, then run a command and inspect the response." |
| Failure or missing information | Calm and factual | "The API returned 403. Check the token's permissions." |
| Product limitation | Brief and explicit | "Saved workflows currently support HTTP commands, not downstream MCP tools." |
| Competitive comparison | Fair and specific | "Compare the same task with the same agent and access." |
| Release note | Explain the change and its effect | "You can now import a schema directly through MCP without creating a temporary file." |

These examples illustrate the voice. Release notes still require checks on the advertised release.

### Wording to keep or cut

| Over-processed | Say it plainly |
| --- | --- |
| "A bounded discovery about the engineer's own environment" | "Answer a question about one service or integration." |
| "Preserve operational understanding across the application lifecycle" | "Keep the commands and checks you'll need again." |
| "Carry validated findings into maintained artifacts" | "Use what you learn in your code, tests, and infrastructure." |
| "Ground agent-assisted implementation in retrieved state" | "Have the agent check the current configuration before changing it." |
| "A unified discovery, experimentation, and operational-assimilation surface" | "One place to explore APIs, try commands, and check results." |

Useful lines include "What does this service connect to?", "Use the same commands as your agent", and "Keep a task you will use again." They name a job without asking the reader to translate the language.

### Review checklist

Before publishing or revising copy:

1. Read it aloud. Would you use these words to explain the tool to another engineer?
2. Can the reader tell what Oatty does and name something to try?
3. Does each claim name an action or result we can demonstrate?
4. Are the agent's reasoning and Oatty's capabilities described accurately?
5. Are the relevant limits clear without burying the useful point?
6. Can you cut a slogan, repeated promise, or qualification without losing meaning?

Apply this to internal marketing documents too. They can use planning and measurement terms, but should define them plainly rather than become copy to repeat everywhere.

## Terminology

Use:

- **Your agent** in public copy; **connected agent** when technical detail requires distinguishing it from Oatty.
- **Stack** for the services behind an application. Use examples to show which services you mean.
- **Catalog** for the persistent collection of commands.
- **Command** or **operation** for an executable API action.
- **API workflow** for a saved sequence of supported HTTP commands.
- **TUI** or **terminal interface** for Oatty's interactive interface.
- **Generated command definition** when explaining exactly what a catalog repair changes.

Use words such as dependencies, boundaries, state, evidence, and verification only when they help explain a specific task.

Avoid or qualify:

- **Agentic**, **execution plane**, and similar category language in introductions.
- **AI-powered Oatty**: Oatty doesn't contain a model.
- **Safe**, **secure**, or **approval gate** without naming the control and execution path.
- **Full coverage**, **every API**, **zero glue**, **automatic**, **instant**, or **no context cost** without proof.
- **Solutions engineering** unless referring to the customer-facing role.
- **Production problems** as the whole promise: setup and ordinary changes matter too.
- **Work across your stack as one system** if it could imply automatic topology or unified state management.
- **What changed** without historical evidence.
- **Blast radius** or **all dependencies** when only some relationships have been checked.

## Surface guidance

### Homepage

Keep the lead, product explanation, and next action close together. Show the three starting questions and a tested example. Make free and open source visible, and keep connection instructions easy to find.

Use **Ask your agent to set up Oatty** and **Copy prompt** for the setup block. Keep its text in sync with [the canonical prompt](../../public/setup-prompt.txt). Keep the prompt to one line and no more than 200 characters so an engineer can paste it without editing. Make any remaining manual step clear; don't promise that every client can complete setup automatically. Keep the prompt readable and manually copyable without JavaScript.

### README and package metadata

Explain what Oatty does early. The README can use the lead and product promise; package metadata should describe the tool directly. Link to API access, schema authorship, repair, and quick start. State tested compatibility and limits where relevant.

### Documentation

Start with the user's task. Use MCP instructions for connecting an agent, the TUI for interactive commands, and the CLI for scripting or CI.

Each procedural page should give the outcome, prerequisites, steps, expected result, common failures, and related next steps.

### Demonstrations

Show real output from a controlled test environment. Label synthetic data and injected failures. Follow the [proof sequence](#proof), identify the resources and environments, and show the values behind a conclusion. Say what couldn't be checked.

For platform-development examples, show the reviewed code or infrastructure and check the deployed result. Ask another engineer to explain and repeat the checks.

### Competitive comparisons

Compare the same task under equivalent conditions. Use primary sources for competitor capabilities. Keep measured results separate from vendor claims and interpretation.

## Accessibility and trust

- Use semantic headings and meaningful links.
- Give screenshots and recordings useful alternative text or captions.
- Show inputs, results, changes, and focus where they help explain the task.
- Keep code, UI text, and captions readable at the intended size.
- Never publish credentials, private endpoints, or raw sensitive logs.
