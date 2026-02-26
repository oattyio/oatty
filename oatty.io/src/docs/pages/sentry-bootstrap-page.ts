import type {DocsPage} from '../types';

/**
 * Sentry bootstrap guide page model.
 *
 * This guide captures real-world validation of Oatty's value proposition:
 * an agent connected through MCP can import a comprehensive OpenAPI schema,
 * discover commands through search, and assemble reviewable workflows that
 * configure an observability platform with minimal prior knowledge.
 */
export const sentryBootstrapPage: DocsPage = {
    path: '/docs/guides/sentry-bootstrap',
    title: 'Bootstrap Sentry with an Agent + MCP',
    summary: 'Import Sentry’s OpenAPI catalog, then use an MCP-connected agent to propose and run reviewable workflows for alerts, dashboards, and hardening.',
    learnBullets: [
        'Import Sentry APIs into Oatty via OpenAPI schema ingestion.',
        'Use search-driven discovery instead of memorizing endpoints.',
        'Turn agent suggestions into explicit, previewable workflows.',
        'Apply org and project tuning safely with human-in-the-loop review.',
        'Link Sentry to PagerDuty and bootstrap Datadog + PagerDuty integrations.',
    ],
    estimatedTime: '15-25 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'overview',
            title: 'What This Proves',
            tocTitle: 'Purpose',
            paragraphs: [
                'Sentry has a comprehensive API surface, but configuring it well typically requires deep prior knowledge of endpoints, scopes, pagination, and failure modes.',
                'In a real-world test, an MCP-connected agent used Oatty to import Sentry APIs, discover the right operations through search, and assemble multi-step workflows for alerts, dashboards, and configuration hardening.',
                'The operator stayed in control: sensitive steps were previewed and confirmed before execution.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can go from “I want good alerts and dashboards” to concrete API-backed changes without first learning Sentry’s full endpoint taxonomy.',
                },
            ],
        },
        {
            id: 'how-to-use-this-guide',
            title: 'How to Use This Guide (Goal-Oriented, NL-First)',
            tocTitle: 'How to Use',
            paragraphs: [
                'This guide is intentionally outcome-focused rather than strictly deterministic. You express goals in natural language; the agent adapts command/workflow selection to your environment.',
                'Treat prompts as control intents, not rigid scripts. Expected actions describe what the agent should accomplish and what evidence it should return.',
                'Consider these safety rules across all runs: no writes before approval, redact secrets in outputs, define rollback before risky changes, and finish with pass/fail confirmation.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can achieve consistent outcomes even when exact command IDs, resources, or provider behavior differ by environment.',
                },
                {
                    type: 'tip',
                    content: 'When a prompt is ambiguous, require the agent to propose a plan first, then request explicit approval before execution.',
                },
            ],
        },
        {
            id: 'why-this-matters',
            title: 'Why This Matters for Real Playbooks',
            tocTitle: 'Value',
            paragraphs: [
                'Complex operational playbooks (for example, error deduping rules, SLO thresholds, and escalation routing) often turn into hours of YAML/JSON editing, permission wrangling, and iterative trial-and-error across multiple tools.',
                'With Oatty connected to an agent through MCP, the “work” shifts from memorizing endpoints to reviewing a proposed workflow: a concrete sequence of API-backed steps with preview, validation, and logs.',
                'The practical benefit is not magic automation: it is faster iteration with fewer accidental misconfigurations because the workflow is explicit and repeatable.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'A successful setup becomes a reusable baseline that can be applied to the next service/team with small edits instead of re-learning and re-clicking.',
                },
                {
                    type: 'tip',
                    content: 'Treat these workflows as “runbooks you can run”: export them, review them, and keep them close to the systems they configure.',
                },
            ],
        },
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            tocTitle: 'Prereqs',
            paragraphs: [
                'Install Oatty and verify `oatty --help` works.',
                'Have a Sentry API token with the scopes required for the operations you intend to perform.',
                'Run Oatty as an MCP server (or connect it as an MCP tool) so your agent can discover and invoke Oatty tooling.',
                'Decide which environment you are targeting (sandbox vs production) and start in a safe environment when possible.',
                'This guide assumes these prerequisites and mandatory auth headers are already fulfilled before you run the prompt patterns.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If you are unsure about token scopes, start by importing the schema and running read-only discovery commands first.',
                },
                {
                    type: 'tip',
                    content: 'Manual setup links: connect MCP via `/docs/learn/mcp-http-server`, review execution model in `/docs/learn/how-oatty-executes-safely`, and review runs in `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'import-sentry-schema',
            title: 'Import Sentry APIs (OpenAPI)',
            tocTitle: 'Import APIs',
            paragraphs: [
                'Import Sentry’s OpenAPI v3 schema as a catalog so Oatty can derive commands.',
                'Use either a local copy of the schema file or a URL to the schema source.',
                'After import, confirm the catalog appears in Library and that commands are discoverable in Run Command and Find.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can search for Sentry operations by intent (for example, “create project”, “list alerts”, “dashboards”).',
                },
                {
                    type: 'recovery',
                    content: 'If import fails, validate the schema is OpenAPI v3 (YAML or JSON) and retry. Use CLI import fallback to capture errors.',
                },
                {
                    type: 'fallback',
                    content: 'CLI import fallback: `oatty import <path-or-url> --kind catalog`.',
                },
            ],
        },
        {
            id: 'configure-auth',
            title: 'Verify Authentication (Bearer Token)',
            tocTitle: 'Authentication',
            paragraphs: [
                'Verify required request headers (for example, `Authorization`) are already set in the catalog headers editor so derived commands can authenticate.',
                'Prefer tokens scoped to the minimum permissions required for your intended changes.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Authenticated discovery and execution calls succeed without embedding tokens in workflows or scripts.',
                },
                {
                    type: 'tip',
                    content: 'Treat catalog headers as configuration. Avoid copying tokens into workflow YAML or repository files.',
                },
                {
                    type: 'tip',
                    content: 'Manual headers setup is covered in `/docs/learn/library-and-catalogs#headers-management`.',
                },
            ],
        },
        {
            id: 'agent-workflows',
            title: 'Let the Agent Propose Workflows (You Still Review)',
            tocTitle: 'Agent Workflows',
            paragraphs: [
                'Use the agent for planning and assembly: it can search commands, suggest sequences, and draft workflows.',
                'When an agent is connected through MCP, the TUI shows interactions, workflow progress, and tool calls in real time while work is being assembled and executed.',
                'Keep execution explicit: validate and preview workflows, then run when the plan matches intent.',
                'This is the key leverage: multi-step configuration becomes a reviewable artifact instead of ad-hoc endpoint guessing.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Use this loop: discover -> draft workflow -> preview/validate -> run -> inspect logs/results -> iterate.',
                },
                {
                    type: 'expected',
                    content: 'If you can review the workflow, you can delegate the assembly work without delegating operator control.',
                },
                {
                    type: 'tip',
                    content: 'For workflow review in the TUI, use Workflows Basics: `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'prompt-patterns-overview',
            title: 'Prompt Patterns: Prompt -> Expected Agent Actions',
            tocTitle: 'Prompt Patterns',
            headingLevel: 2,
            paragraphs: [
                'Use these operator-ready prompt patterns when bootstrapping Sentry with an MCP-connected agent.',
                'Each pattern keeps execution reviewable: prompt in a code block, expected behavior in callouts.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If workflow names differ in your environment, ask the agent to discover existing workflows first and substitute exact IDs.',
                },
            ],
        },
        {
            id: 'prompt-pattern-1-import-and-verify',
            title: 'Pattern 1: Import Sentry APIs and Verify Headers',
            tocTitle: 'Pattern 1: Import',
            headingLevel: 3,
            paragraphs: [
                'Use this first when starting in a new Sentry org or environment.',
            ],
            codeSample: `Prompt:
Import Sentry APIs into Oatty using the OpenAPI schema URL (preferred). Verify required auth headers (for example, Authorization) are already configured, then summarize imports and readiness for authenticated discovery.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: import the catalog from URL, verify required headers are present, and report readiness for authenticated discovery.',
                },
            ],
        },
        {
            id: 'prompt-pattern-2-discover-and-draft',
            title: 'Pattern 2: Discover Commands and Draft Workflow',
            tocTitle: 'Pattern 2: Draft',
            headingLevel: 3,
            paragraphs: [
                'Use this when your goal is clear but exact endpoints are unknown.',
            ],
            codeSample: `Prompt:
Discover the commands needed to bootstrap Sentry for this service (project setup, alerting baseline, dashboard baseline). Draft a reviewable workflow and ask me for any missing inputs that cannot be derived.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: search for commands by intent, inspect current state, draft a workflow with validation gates, and request missing inputs before writes.',
                },
            ],
        },
        {
            id: 'prompt-pattern-3-preflight-before-write',
            title: 'Pattern 3: Preflight Gate Before Mutations',
            tocTitle: 'Pattern 3: Preflight',
            headingLevel: 3,
            paragraphs: [
                'Use this to force safe-stop behavior before any configuration changes.',
            ],
            codeSample: `Prompt:
Before any write action, run a read-only preflight and stop if permissions or required identifiers are missing.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run read-only checks first, enforce fail-fast gates, and return concrete remediation steps instead of attempting writes.',
                },
            ],
        },
        {
            id: 'prompt-pattern-4-verify-and-summarize',
            title: 'Pattern 4: Verify Outcomes and Summarize Risks',
            tocTitle: 'Pattern 4: Verify',
            headingLevel: 3,
            paragraphs: [
                'Use this after initial setup or when preparing a status report.',
            ],
            codeSample: `Prompt:
Re-run integration verification for this Sentry baseline and produce a pass/fail checklist with unresolved risks and recommended next actions.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run verification workflows, emit a checklist for configured resources, and summarize outstanding risk items.',
                },
                {
                    type: 'tip',
                    content: 'Review workflow details in the Workflows view: `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'what-the-agent-can-handle',
            title: 'What the Agent Can Handle Through Oatty',
            tocTitle: 'Agent Capabilities',
            paragraphs: [
                'The following categories are representative of the “expertise barrier” reduction you get when an agent can discover and execute via Oatty:',
                'Alerts and monitors: create, update, and tune alert rules and monitor configurations.',
                'Alert workflows: build multi-step routing and notification flows.',
                'Dashboards: create and update dashboard definitions programmatically.',
                'Org-level hardening: apply global settings changes (for example, SSO/SCIM configuration, rate limit and security-related settings) with careful review.',
                'Project tuning: adjust project-level settings (for example, grouping and filters) and align SDK configuration expectations.',
                'Add Sentry as a project source: coordinate repo/service integrations by combining Oatty workflows with other MCP tools (for example, GitHub or filesystem tooling) when available.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can translate ambiguous intent into concrete, logged, repeatable operations without becoming an API expert first.',
                },
                {
                    type: 'advanced',
                    content: 'Use workflow export/import to share reviewed configuration playbooks with teammates and CI pipelines.',
                },
            ],
        },
        {
            id: 'link-pagerduty-to-sentry',
            title: 'Next: Link PagerDuty to Sentry',
            tocTitle: 'Link PagerDuty',
            paragraphs: [
                'Once Sentry is bootstrapped, a high-leverage next step is connecting incident response by linking PagerDuty.',
                'In practice, this is typically a multi-resource flow: create or select PagerDuty services, configure an integration (or event routing), then configure Sentry to send the right alerts to the right targets.',
                'With Oatty connected through MCP, your agent can discover the necessary operations in both APIs and draft a workflow that you review before execution.',
                'Use the Sentry + Datadog + PagerDuty playbook for the concrete command sequence and rollback steps.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Start with read-only discovery in both catalogs (list services/integrations/routing rules) to confirm identifiers and permissions before creating or changing anything.',
                },
                {
                    type: 'expected',
                    content: 'You end up with an explicit workflow that sets up the PagerDuty linkage and can be re-run (or adapted) for new teams/projects.',
                },
                {
                    type: 'recovery',
                    content: 'If setup fails due to permissions or resource constraints, reduce the workflow to the smallest failing step, adjust scopes/roles, validate, then continue.',
                },
                {
                    type: 'fallback',
                    content: 'Follow the playbook: `/docs/guides/sentry-datadog-pagerduty-playbook`.',
                },
            ],
        },
        {
            id: 'bootstrap-datadog-and-pagerduty',
            title: 'Then: Bootstrap Datadog + PagerDuty Integrations',
            tocTitle: 'Bootstrap Datadog',
            paragraphs: [
                'After Sentry → PagerDuty is in place, you can use the same discovery-to-workflow loop to bootstrap Datadog monitors and route events to PagerDuty.',
                'This is a classic “expertise barrier” problem: monitor types, query languages, notification policies, paging rules, and team ownership are all separate concepts with separate endpoints.',
                'Oatty helps by turning OpenAPI schemas into runnable commands and letting an agent propose cohesive, reviewable workflows across multiple systems.',
                'Use the Sentry + Datadog + PagerDuty playbook for safe validation commands (payload validation, draft monitor creation, and rollbacks).',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can go from intent (“page on high error rate”, “dashboard per service”, “route to on-call”) to a repeatable baseline without spending days learning three different APIs.',
                },
                {
                    type: 'advanced',
                    content: 'If your agent has access to additional MCP tools (for example, GitHub), extend the workflow to store exported manifests and update runbooks or service metadata in-repo.',
                },
                {
                    type: 'fallback',
                    content: 'Follow the playbook: `/docs/guides/sentry-datadog-pagerduty-playbook`.',
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            tocTitle: 'Next Steps',
            paragraphs: [
                'If you have not already, read How Oatty Executes Safely for the trust model and operator control pattern.',
                'Use MCP HTTP Server docs to connect your agent reliably.',
                'Use Workflows Basics to convert successful sequences into reusable YAML definitions.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can repeat this approach for other observability platforms (for example, Datadog and PagerDuty) by importing their OpenAPI catalogs and applying the same discovery-to-workflow loop.',
                },
            ],
        },
    ],
};
