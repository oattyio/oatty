import type {DocsPage} from '../types';

/**
 * Sentry + Datadog + PagerDuty integration playbook page model.
 *
 * This page is a structured version of the internal runbook used to repeatedly
 * configure and validate the integration chain:
 *
 * - Sentry -> PagerDuty (native integration actions)
 * - Datadog -> PagerDuty (service-key mapping via Datadog integration)
 * - Sentry -> Datadog (provider availability check)
 *
 * The goal is a repeatable, auditable workflow that an MCP-connected agent can
 * draft and an operator can explicitly review and run.
 */
export const sentryDatadogPagerDutyPlaybookPage: DocsPage = {
    path: '/docs/guides/sentry-datadog-pagerduty-playbook',
    title: 'Sentry + Datadog + PagerDuty Integration Playbook',
    summary: 'A natural-language, agent-driven procedure to configure, verify, and roll back Sentry → PagerDuty and Datadog → PagerDuty integrations through Oatty.',
    learnBullets: [
        'Connect Oatty to a natural-language agent via MCP.',
        'Import OpenAPI catalogs and use preconfigured auth headers.',
        'Prompt the agent to draft safe, reviewable workflows for integration setup.',
        'Confirm provider availability and required identifiers.',
        'Validate PagerDuty service + Events API v2 integration details.',
        'Create and verify Datadog → PagerDuty service-key mappings.',
        'Validate notification handles safely and keep monitors in draft mode.',
        'Roll back cleanly when needed.',
    ],
    estimatedTime: '20-35 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'purpose',
            title: 'Purpose',
            tocTitle: 'Purpose',
            paragraphs: [
                'This playbook documents a repeatable integration style and a safe procedure to configure, verify, and roll back:',
                'Sentry -> PagerDuty',
                'Datadog -> PagerDuty',
                'Sentry -> Datadog (provider availability check)',
                'This version is written for a natural-language workflow: you prompt an MCP-connected agent, the agent drafts commands/workflows in Oatty, and you explicitly review and run.',
                'Use this when onboarding a new environment or re-running setup after service renames, org changes, or key rotation.',
                'This guide assumes prior account setup, access to credentials (PAT or Bearer token) and the ability to set these tokens securely in Oatty',
            ],
            callouts: [
                {
                    type: 'tip',
                    label: 'MCP setup',
                    content: 'If your agent is not connected yet, set up the MCP server first: `/docs/learn/mcp-http-server`.',
                },
            ],
        },
        {
            id: 'how-to-use-this-guide',
            title: 'How to Use This Guide (Goal-Oriented, NL-First)',
            tocTitle: 'How to Use',
            paragraphs: [
                'This guide is designed to communicate goals rather than define one exact path. Use prompts to express intent; let the agent adapt commands/workflows for your environment.',
                'Prompt patterns are non-deterministic by design: they define expected outcomes and guardrails, not fixed command sequences for every account/org. Always verify output from your agent before execution.',
                'Keep these safety rules in mind: explicit approval before writes, never provide secrets to your agent, rollback readiness before risky changes, and pass/fail confirmation after each phase.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Different environments can use different commands while still achieving on the same integration outcomes.',
                },
                {
                    type: 'tip',
                    content: 'If command IDs or workflow IDs differ, require your agent to perform discovery first, then continue only after the agent maps equivalent actions for your environment.',
                },
            ],
        },
        {
            id: 'prompt-patterns-overview',
            title: 'Prompt Patterns: Prompt -> Expected Agent Actions',
            tocTitle: 'Prompt Patterns',
            headingLevel: 2,
            paragraphs: [
                'Use these prompt patterns when working with an MCP-connected agent. Adjust the prompts as needed to achieve the desired outcome.',
                'Each pattern includes a prompt block plus expected actions so review remains predictable and auditable.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Workflow IDs differ by environment. Ask the agent to discover available workflows first, or view them in Oatty\'s TUI workflow view.',
                },
                {
                    type: 'expected',
                    content: 'Each prompt should result in accurate, valid workflows, clear gates, and auditable outcomes.',
                },
                {
                    type: 'advanced',
                    content: 'Ask your agent to use value providers for inputs to your workflow. This will data to be retrieved remotely as options to workflow input when running it manually.',
                },
            ],
        },
        {
            id: 'prompt-pattern-1-import-catalogs',
            title: 'Pattern 1: Import Catalogs and Verify Headers',
            tocTitle: 'Pattern 1: Import',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern at the start of a new environment onboarding to automate the catalog import process.',
            ],
            codeSample: `Prompt:
Import APIs for Sentry, Datadog, and PagerDuty into Oatty using their OpenAPI schemas (URLs preferred). Stub the necessary headers (for example, Authorization), then pause for me to enter access tokens in Oatty\'s TUI.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: import published OpenAPI catalogs, places blank values for any required headers, and pauses for token entry.',
                },
                {
                    type: 'tip',
                    content: 'The Oatty TUI logs agent actions in real time. Use `Ctrl+L` in Oatty to view the logs panel and follow along.',
                },
            ],
        },
        {
            id: 'prompt-pattern-2-configure-mapping',
            title: 'Pattern 2: Configure Mapping with Review Gates',
            tocTitle: 'Pattern 2: Configure',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern when you want the agent to begin the process of discovery and planning end-to-end.',
            ],
            codeSample: `Prompt:
Configure Sentry -> Datadog -> PagerDuty mapping using workflows. Ask me for any missing inputs or actions needed to proceed that cannot be accomplished via Oatty.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: verify connectivity, discover commands, inspect current state, draft preflight/config workflows for review, map outputs, and report success/failure with rollback status.',
                },
            ],
        },
        {
            id: 'prompt-pattern-3-non-paging-validation',
            title: 'Pattern 3: Non-Paging Datadog -> PagerDuty Validation',
            tocTitle: 'Pattern 3: Validate',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern to validate routing safely without sending pages.',
            ],
            codeSample: `Prompt:
Create a non-paging validation path for Datadog -> PagerDuty using workflows.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run the environment\'s validation workflow, confirm monitor is draft, confirm PagerDuty handle in monitor message, and return monitor ID with a safe publish/test plan.',
                },
            ],
        },
        {
            id: 'prompt-pattern-4-preflight-stop',
            title: 'Pattern 4: Preflight Stop-Before-Write Gate',
            tocTitle: 'Pattern 4: Preflight',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern to force hard stop behavior before any write path.',
            ],
            codeSample: `Prompt:
Before any write action, run preflight workflow and stop if PagerDuty service is missing or Datadog auth fails.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: execute preflight, enforce gate conditions, and halt on failure with concrete remediation steps instead of attempting writes.',
                },
            ],
        },
        {
            id: 'prompt-pattern-5-verification-checklist',
            title: 'Pattern 5: Re-Run Verification with Checklist',
            tocTitle: 'Pattern 5: Checklist',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern after setup changes or before release windows.',
            ],
            codeSample: `Prompt:
Re-run full integration verification and produce a pass/fail checklist.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run preflight plus Sentry/PagerDuty audit workflows and emit a checklist for Datadog mapping, Sentry PagerDuty wiring, draft monitor presence, and unresolved risks.',
                },
            ],
        },
        {
            id: 'prompt-pattern-6-idempotent-configure-then-verify',
            title: 'Pattern 6: Idempotent Configure-Then-Verify',
            tocTitle: 'Pattern 6: Idempotent',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern when existing configuration may already be present.',
            ],
            codeSample: `Prompt:
Use workflows to configure and then validate; if configuration already exists, skip creation and only verify.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run preflight first, detect existing Datadog mapping, skip create when present, still run verification, and return an idempotent outcome summary.',
                },
                {
                    type: 'tip',
                    content: 'Ask the agent to assess an existing Sentry, Datadog or PagerDuty configuration and surface gaps or areas of concern.',
                },
            ],
        },
        {
            id: 'prompt-pattern-7-secrets-hardening',
            title: 'Pattern 7: Workflow Output Hardening',
            tocTitle: 'Pattern 7: Hardening',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern before sharing manifests across teams or repositories.',
            ],
            codeSample: `Prompt:
Harden workflow outputs so secrets are never exposed. Review and patch workflow manifests accordingly.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: inspect manifests, remove steps/flags that expose secret-bearing fields, sanitize examples/placeholders, save, and revalidate workflows.',
                },
            ],
        },
        {
            id: 'prompt-pattern-8-export-and-run-order',
            title: 'Pattern 8: Export Workflows with Run Order',
            tocTitle: 'Pattern 8: Export',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern to produce operational documentation such as a README.md.',
            ],
            codeSample: `Prompt:
Export all integration workflows to docs/workflows and include a run-order README with important notes.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: export runtime workflows into the repository, verify files exist, add execution order and safety notes, and report exact file paths.',
                },
            ],
        },
        {
            id: 'prompt-pattern-9-read-only-summary',
            title: 'Pattern 9: Read-Only Executive Summary',
            tocTitle: 'Pattern 9: Read-Only',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern when leadership status is needed without changes to configuration or resources.',
            ],
            codeSample: `Prompt:
Run only read-only workflows to extract data and provide an executive summary for leadership.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: execute only preflight/audit workflows and provide status, key risks, and next actions with no writes.',
                },
                {
                    type: 'tip',
                    content: 'Some agents support scheduled cron jobs for repeated report generation. Oatty workflows allow agents to derive data deterministically in scheduled jobs.',
                },
            ],
        },
        {
            id: 'prompt-pattern-10-controlled-e2e',
            title: 'Pattern 10: Controlled End-to-End Test',
            tocTitle: 'Pattern 10: Controlled Test',
            headingLevel: 3,
            paragraphs: [
                'Use this pattern for controlled test windows with explicit approval checkpoints.',
            ],
            codeSample: `Prompt:
Perform a controlled end-to-end test with explicit approval gates: configure, validate draft monitor, optional publish, optional rollback.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: stage workflow runs in sequence, pause before paging-capable steps, execute only approved gates, and return to a safe state (draft/delete) when requested.',
                },
                {
                    type: 'tip',
                    content: 'For reviewing any generated workflow in the TUI, use `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'accounts',
            title: 'Verify Accounts and Access (Manual Check)',
            tocTitle: 'Prereqs',
            paragraphs: [
                'Before prompting an agent, confirm the accounts and permissions exist for Sentry, Datadog, and PagerDuty.',
                'Your agent can check the existence of headers attached to a catalog but Oatty never provides values. Values mut be input by the user before commands can be run. The rest of the playbook assumes you can authenticate to each API with the needed scopes and that required auth headers are already set.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You have valid credentials/tokens for all three systems and you know which org/account you are targeting.',
                },
                {
                    type: 'tip',
                    content: 'Start in a sandbox/non-production environment when possible, then repeat in production after the workflow is reviewed.',
                },
                {
                    type: 'tip',
                    content: 'Useful links: MCP setup `/docs/learn/mcp-http-server`, workflow review `/docs/learn/workflows-basics`, and execution guardrails `/docs/learn/how-oatty-executes-safely`.',
                },
            ],
        },
        {
            id: 'connect-oatty-to-agent',
            title: 'Connect Oatty to Your Agent (MCP)',
            tocTitle: 'MCP Setup',
            paragraphs: [
                'Expose Oatty to your agent through MCP so the agent can discover Oatty tools and request preview/validation/execution flows.',
                'With an active MCP connection, the TUI shows interactions, workflow state changes, and tool calls in real time so you can observe execution as it happens.',
                'Keep the trust model: the agent can propose and create, but execution remains explicit and observable.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Follow the MCP server setup page: `/docs/learn/mcp-http-server`.',
                },
                {
                    type: 'tip',
                    content: 'Review the trust model for assisted execution: `/docs/learn/how-oatty-executes-safely`.',
                },
                {
                    type: 'tip',
                    content: 'Review proposed and running workflows in the Workflows view; details are covered in `/docs/learn/workflows-basics`.',
                },
                {
                    type: 'advanced',
                    content: 'Asking your agent to execute read-only commands for discovery is a common use case and can lead to better overall accuracy and guidance.',
                },
            ],
        },
        {
            id: 'agent-imports',
            title: 'Prompt the Agent to Import APIs and Verify Headers',
            tocTitle: 'Import APIs',
            paragraphs: [
                'Use a short prompt to have the agent import OpenAPI schemas for Sentry, Datadog, and PagerDuty into Oatty as catalogs.',
                'Ask it to verify the necessary headers (for example, `Authorization`) are present so authenticated commands are ready without exposing secrets in chat or workflows.',
            ],
            codeSample: `Prompt:
Import APIs for Sentry, Datadog, and PagerDuty into Oatty using their OpenAPI schemas (URLs or local paths). Verify required auth headers (e.g., Authorization) are configured for each catalog, then summarize what was imported and what is ready.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Three catalogs are imported and you can search commands for each vendor immediately.',
                },
                {
                    type: 'recovery',
                    content: 'If an OpenAPI schema import fails, have the agent try a local file, confirm OpenAPI v3, or import just one vendor at a time to narrow the error.',
                },
            ],
        },
        {
            id: 'add-tokens-in-tui',
            title: 'Verify Headers in the Oatty TUI (Catalog Headers)',
            tocTitle: 'Set Headers',
            paragraphs: [
                'In the Library view, verify each imported catalog has required auth header values already set.',
                'Keep secrets out of workflow YAML and out of agent prompts while still enabling authenticated discovery and execution.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'See the headers editor flow: `/docs/learn/library-and-catalogs#headers-management`.',
                },
                {
                    type: 'expected',
                    content: 'Authenticated read-only calls succeed (list orgs/projects/services) before you attempt any mutating setup steps.',
                },
                {
                    type: 'tip',
                    content: 'Use minimum scopes first. Expand permissions only when you hit a specific blocked step.',
                },
            ],
        },
        {
            id: 'value-of-workflow-shape',
            title: 'Why This Is Written as a Playbook',
            tocTitle: 'Why Playbook',
            paragraphs: [
                'Integrations tend to fail in the “last mile”: permissions, identifier mismatches, missing providers, and unsafe test notifications.',
                'A playbook reduces iteration time by making the sequence explicit and by separating safe validation from mutating steps.',
                'This same structure scales to more complex configuration work (for example, error deduping, SLO thresholds, and escalation routing), where manual YAML/JSON editing and repeated trial runs are otherwise the default.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You get a repeatable baseline with clear verification points and rollbacks, which reduces alert fatigue and missed incidents caused by subtle misconfiguration.',
                },
                {
                    type: 'tip',
                    content: 'When you expand this beyond integrations, keep the same pattern: preflight -> validate -> draft/safe create -> controlled enablement -> verify -> rollback.',
                },
                {
                    type: 'tip',
                    label: 'Agent prompt',
                    content: 'Ask the agent to keep mutating steps behind explicit validation/preview, and to start with draft or non-notifying configurations.',
                },
            ],
        },
        {
            id: 'integration-style',
            title: 'Current Integration Style',
            tocTitle: 'Integration Style',
            paragraphs: [
                'Sentry <-> PagerDuty: native Sentry PagerDuty integration actions.',
                'Datadog <-> PagerDuty: service-key mapping via Datadog PagerDuty integration.',
                'Sentry <-> Datadog: no native provider was available in the verified org at the time of testing; treat this as a provider availability check.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Sentry provider availability can differ by org. Always check before assuming a native Sentry → Datadog install flow exists.',
                },
            ],
        },
        {
            id: 'preflight',
            title: 'Preflight',
            tocTitle: 'Preflight',
            paragraphs: [
                'Confirm Oatty provider auth is valid for `sentry`, `datadog`, and `pagerduty` catalogs/plugins.',
                'Confirm target identifiers (at minimum): Sentry org slug and PagerDuty service name.',
                'Never store PagerDuty integration keys in git. Treat them as secrets and store them using your secrets backend.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'For a new environment, start by running read-only commands to validate auth/scopes before drafting any workflow that creates or mutates resources.',
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "List the identifiers we need (org slug, service ID, integration ID) and propose read-only commands to fetch them in each system."',
                },
            ],
        },
        {
            id: 'sentry-provider-availability',
            title: 'Confirm Sentry Integration Provider Availability',
            tocTitle: 'Sentry Providers',
            paragraphs: [
                'Confirm that Sentry has the expected integration providers available in the target org.',
                'You want to see PagerDuty present. Datadog may or may not exist.',
            ],
            codeSample: `# List integration providers for an org
sentry organizations:config:integrations:list <ORG_SLUG>

# Optional targeted check
sentry organizations:config:integrations:list <ORG_SLUG> --providerKey datadog`,
            callouts: [
                {
                    type: 'expected',
                    content: '`pagerduty` provider exists. If `datadog` provider is missing, treat native Sentry → Datadog install as unavailable.'
                },
                {
                    type: 'recovery',
                    content: 'If a provider is missing, use alternate forwarding patterns instead of trying to force a native install flow.'
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "Check which integration providers exist for this Sentry org and summarize what is available for PagerDuty and Datadog."',
                },
            ],
        },
        {
            id: 'pagerduty-service-and-events-integration',
            title: 'Confirm PagerDuty Service + Events API v2 Integration',
            tocTitle: 'PagerDuty Service',
            paragraphs: [
                'Identify the target PagerDuty service.',
                'Fetch integrations for the service and locate the Events API v2 integration (integration key is a secret).',
            ],
            codeSample: `# List services
pagerduty services:list

# Fetch service info including integrations
pagerduty services:info <SERVICE_ID> --include[]=integrations

# Fetch integration info (use integration ID from previous output)
pagerduty services:integrations:info <SERVICE_ID> <INTEGRATION_ID>`,
            callouts: [
                {
                    type: 'expected',
                    content: 'You can capture `service_name` and the Events API v2 `integration_key` (keep secret).',
                },
                {
                    type: 'tip',
                    content: 'Prefer unambiguous service names to reduce routing mistakes and future maintenance risk.',
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "Find the PagerDuty service we should route to, list its integrations, and tell me which one is Events API v2."',
                },
            ],
        },
        {
            id: 'datadog-mapping',
            title: 'Configure Datadog -> PagerDuty Mapping',
            tocTitle: 'Datadog Mapping',
            paragraphs: [
                'Create the service-key mapping in Datadog using the PagerDuty service name and Events API v2 integration key.',
                'Then verify the mapping exists.',
            ],
            codeSample: `# Create mapping
datadog api:integration:pagerduty:configuration:services:create \\
  --service_name "<PAGERDUTY_SERVICE_NAME>" \\
  --service_key "<PAGERDUTY_EVENTS_API_V2_INTEGRATION_KEY>"

# Verify mapping
datadog api:integration:pagerduty:configuration:services:info "<PAGERDUTY_SERVICE_NAME>"`,
            callouts: [
                {type: 'expected', content: 'The mapping exists and the response contains the target `service_name`.'},
                {
                    type: 'recovery',
                    content: 'If creation fails, re-check token scopes/permissions and confirm the integration key is correct for the target service.'
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "Create (or update) the Datadog PagerDuty service mapping for <service_name>. Then verify it exists."',
                },
            ],
        },
        {
            id: 'validate-handle-safe',
            title: 'Validate Datadog Notification Handle (No Paging)',
            tocTitle: 'Validate Handle',
            paragraphs: [
                'Validate a monitor payload without creating a monitor or paging.',
                'This is a safe check for message format and handle resolution.',
            ],
            codeSample: `datadog api:monitor:validate:create \\
  --type "query alert" \\
  --query "avg(last_5m):avg:system.load.1{*} > 100000" \\
  --name "integration-validation-pd-handle" \\
  --message "Datadog->PagerDuty validation @pagerduty-<SERVICE-NAME-HANDLE>" \\
  --options '{\"notify_no_data\":false,\"thresholds\":{\"critical\":100000}}'`,
            callouts: [
                {type: 'tip', content: 'Recommended handle format example: `@pagerduty-Default-Service`.'},
                {type: 'expected', content: 'Payload validation succeeds without creating a monitor.'},
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "Validate a Datadog monitor payload that would route to PagerDuty, without creating a monitor or notifying."',
                },
            ],
        },
        {
            id: 'draft-monitor',
            title: 'Create a Draft Datadog Validation Monitor',
            tocTitle: 'Draft Monitor',
            paragraphs: [
                'Create a monitor in draft mode so it does not notify.',
                'This provides a persisted artifact you can inspect and later enable under controlled incident testing.',
            ],
            codeSample: `datadog api:monitor:create \\
  --name "integration-validation datadog-to-pagerduty" \\
  --type "query alert" \\
  --query "avg(last_5m):avg:system.load.1{*} > 100000" \\
  --message "Datadog to PagerDuty validation @pagerduty-Default-Service" \\
  --tags '[\"integration:datadog-pagerduty\",\"env:production\",\"managed-by:oatty\"]' \\
  --priority 3 \\
  --draft_status draft \\
  --options '{\"notify_no_data\":false,\"include_tags\":true,\"thresholds\":{\"critical\":100000}}'

# Verify
datadog api:monitor:info <MONITOR_ID>`,
            callouts: [
                {
                    type: 'expected',
                    content: '`draft_status` is `draft`, and the message contains the PagerDuty handle.'
                },
                {
                    type: 'tip',
                    content: 'Keep validation monitors in draft unless you are running a controlled incident test.'
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "Create a Datadog validation monitor in draft mode with the PagerDuty handle, then fetch it and confirm draft_status."',
                },
            ],
        },
        {
            id: 'sentry-pagerduty-validation',
            title: 'Validate Sentry -> PagerDuty Actions',
            tocTitle: 'Sentry -> PagerDuty',
            paragraphs: [
                'Verify at least one Sentry alert rule or workflow includes a PagerDuty action with a non-null integration identifier.',
                'This confirms Sentry is wired to incident response in a concrete, inspectable way.',
            ],
            codeSample: `# Alert rules
sentry organizations:alert-rules:list <ORG_SLUG>

# Workflows (optional)
sentry organizations:workflows:list <ORG_SLUG> --project [<PROJECT_ID>]`,
            callouts: [
                {type: 'expected', content: 'You see action type `pagerduty` and a non-null integration identifier.'},
                {
                    type: 'recovery',
                    content: 'If no PagerDuty actions exist, create or edit a rule/workflow and add a PagerDuty action via the integration.'
                },
                {
                    type: 'tip',
                    label: 'Minimal prompt',
                    content: 'Prompt: "List Sentry alert rules/workflows and confirm at least one routes to PagerDuty. If none do, propose the smallest safe change to add one."',
                },
            ],
        },
        {
            id: 'rollback',
            title: 'Rollback',
            tocTitle: 'Rollback',
            paragraphs: [
                'Datadog PagerDuty mapping rollback:',
                'Datadog validation monitor rollback:',
                'Sentry PagerDuty rollback:',
            ],
            codeSample: `# Datadog PagerDuty mapping rollback
datadog api:integration:pagerduty:configuration:services:delete "<PAGERDUTY_SERVICE_NAME>"

# Datadog validation monitor rollback
datadog api:monitor:delete <MONITOR_ID>

# Sentry PagerDuty rollback
# Remove PagerDuty actions from rules/workflows in Sentry, or disable impacted workflows.`,
            callouts: [
                {type: 'tip', content: 'Test rollback paths in non-production first.'},
            ],
        },
        {
            id: 'rerun-checklist',
            title: 'Re-run Checklist',
            tocTitle: 'Re-run Checklist',
            paragraphs: [
                'Provider auth confirmed.',
                'PagerDuty service and integration key confirmed.',
                'Datadog mapping created or updated.',
                'Datadog monitor payload validates.',
                'Draft monitor exists and is query-valid.',
                'Sentry rules/workflows include PagerDuty actions.',
                'Rollback commands tested in non-production first.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can re-run this playbook safely after org migrations, service renames, or key rotation.'
                },
            ],
        },
    ],
};
