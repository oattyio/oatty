import type {DocsPage} from '../types';

/**
 * Access review and incident evidence collection playbook page model.
 *
 * This playbook focuses on real-world, read-only collection across providers
 * with explicit preflight gates and partial-results reporting.
 */
export const accessReviewCollectionPlaybookPage: DocsPage = {
    path: '/docs/guides/access-review-collection-playbook',
    title: 'Access Review Collection Playbook (Okta + AWS IAM + Datadog)',
    summary: 'A prompt-driven, agent-assisted guide to collect user lists, permission grants, and audit evidence for quarterly reviews or incidents using read-only workflows.',
    learnBullets: [
        'If required catalogs are missing, ask the agent to import them and verify auth headers first.',
        'Run provider connectivity preflight before any collection.',
        'Collect identity and access posture with read-only commands/workflows.',
        'Normalize outputs into a single review-ready report shape.',
        'Handle partial provider failures with explicit gap reporting.',
        'Produce quarterly and incident summaries without exposing secrets.',
    ],
    estimatedTime: '15-25 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'purpose',
            title: 'Purpose',
            tocTitle: 'Purpose',
            paragraphs: [
                'This playbook is for collecting access-review evidence from Okta, AWS IAM, and Datadog through an MCP-connected agent.',
                'Use it for quarterly access reviews and incident response evidence collection when you need auditable, repeatable, read-only execution.',
                'The guide is goal-oriented and non-deterministic: prompts define outcomes and safety constraints, while command/workflow selection adapts to each environment.',
                'If required provider catalogs or auth headers are not yet configured, begin with an import-and-verify step before running collection prompts.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Before running prompts, ensure your agent is connected through MCP HTTP Server: `/docs/learn/mcp-http-server`.',
                },
                {
                    type: 'tip',
                    content: 'If you still need to add or update required auth headers manually, use `/docs/learn/library-and-catalogs#headers-management` first.',
                },
                {
                    type: 'tip',
                    content: 'Use the Workflows view to review run plans, approvals, and execution details: `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'scope',
            title: 'Scope and Feasibility',
            tocTitle: 'Scope',
            paragraphs: [
                'In-scope outcomes for this playbook:',
                '1) User and group inventory (identity state).',
                '2) Role/policy/grant inventory (permission state).',
                '3) Available audit/security event evidence per provider.',
                '4) Unified summary with pass/fail coverage and unresolved gaps.',
                'With provider authentication already configured, this guide is feasible for identity and permission collection across Okta, AWS IAM, and Datadog. Audit/event depth varies by provider endpoint availability and account configuration.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected result: a useful guide remains feasible even when one provider is partially unavailable, as long as preflight and gap reporting are enforced.',
                },
            ],
        },
        {
            id: 'validated-signals',
            title: 'Validated Read-Only Signals',
            tocTitle: 'Validated Signals',
            paragraphs: [
                'Representative evidence signals available through Oatty command catalogs:',
                'Identity inventory: `github users:list`, `okta api:users:list`, `okta api:groups:list`, `aws #Action=ListUsers:list`, `datadog users:list`.',
                'Permission posture: `aws #Action=ListRoles:list`, `aws #Action=ListPolicies:list`, `aws #Action=GetAccountAuthorizationDetails:list`, `datadog permissions:list`, `github repos:collaborators:list`.',
                'Audit/event evidence: `okta api:logs:list` and provider-specific event endpoints where available.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'This playbook remains useful even with partial provider availability, as long as outputs include explicit provider coverage and collection gaps.',
                },
            ],
        },
        {
            id: 'how-to-use',
            title: 'How to Use This Guide (Goal-Oriented, NL-First)',
            tocTitle: 'How to Use',
            paragraphs: [
                'Start each run with clear goals, time window, and allowed mutation policy (read-only by default).',
                'Have the agent discover available commands/workflows first, then run preflight gates, then collect evidence in provider order.',
                'Require normalized output sections and explicit coverage gaps rather than raw payload dumps.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If you need a refresher on command discovery and safe execution controls, see `/docs/learn/search-and-run-commands` and `/docs/learn/how-oatty-executes-safely`.',
                },
            ],
        },
        {
            id: 'prompt-patterns',
            title: 'Prompt Patterns: Prompt -> Expected Agent Actions',
            tocTitle: 'Prompt Patterns',
            headingLevel: 2,
            paragraphs: [
                'Use these prompt patterns to drive consistent collection and reporting behavior with a connected agent.',
            ],
        },
        {
            id: 'pattern-preflight',
            title: 'Pattern 1: Auth and Connectivity Preflight',
            tocTitle: 'Pattern 1: Preflight',
            headingLevel: 3,
            paragraphs: [
                'Run this first on every quarterly or incident pull.',
            ],
            codeSample: `Prompt:
Run a preflight for Okta, AWS IAM, and Datadog before we collect anything. If a catalog is missing, import it first, verify required headers, and give me a go/no-go summary.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: import missing catalogs when needed, verify required headers, execute provider connectivity checks, and produce a go/no-go table before evidence collection.',
                },
            ],
        },
        {
            id: 'pattern-collect-identity',
            title: 'Pattern 2: Collect Identity Inventory',
            tocTitle: 'Pattern 2: Identity',
            headingLevel: 3,
            paragraphs: [
                'Use this to collect users, groups, and account-level ownership context.',
            ],
            codeSample: `Prompt:
Collect a read-only identity inventory from Okta, AWS IAM, and Datadog. Return users, groups/teams, and account ownership in one normalized format.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run read-only identity commands per provider, map provider-specific fields to a shared schema, and return counts plus key identifiers.',
                },
                {
                    type: 'fallback',
                    content: 'Representative commands include `okta api:users:list`, `okta api:groups:list`, `aws #Action=ListUsers:list`, `datadog users:list`, and `datadog roles:list`.',
                },
            ],
        },
        {
            id: 'pattern-collect-permissions',
            title: 'Pattern 3: Collect Permissions and Grants',
            tocTitle: 'Pattern 3: Grants',
            headingLevel: 3,
            paragraphs: [
                'Use this to capture effective permission posture and high-risk grant surfaces.',
            ],
            codeSample: `Prompt:
Collect read-only permission grants and role/policy mappings from Okta, AWS IAM, and Datadog. Highlight admin-equivalent access and anything with unclear ownership.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: gather roles/policies/grants, classify privilege level, and flag high-risk assignments for reviewer follow-up.',
                },
                {
                    type: 'fallback',
                    content: 'Representative commands include `aws #Action=ListRoles:list`, `aws #Action=ListPolicies:list`, `aws #Action=GetAccountAuthorizationDetails:list`, and `datadog permissions:list`.',
                },
            ],
        },
        {
            id: 'pattern-collect-audit',
            title: 'Pattern 4: Collect Audit and Event Evidence',
            tocTitle: 'Pattern 4: Audit',
            headingLevel: 3,
            paragraphs: [
                'Use this to collect available audit/security events for the requested time window.',
            ],
            codeSample: `Prompt:
Pull read-only audit/security evidence for the last 90 days from available provider endpoints. Summarize key actions and call out any coverage gaps.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: collect available event streams, summarize notable admin/security actions, and explicitly report provider-specific audit depth limits.',
                },
                {
                    type: 'fallback',
                    content: 'For example, use `okta api:logs:list` where available and account for missing endpoints by emitting a structured collection gap section.',
                },
            ],
        },
        {
            id: 'pattern-quarterly-summary',
            title: 'Pattern 5: Quarterly Review Output',
            tocTitle: 'Pattern 5: Quarterly',
            headingLevel: 3,
            paragraphs: [
                'Use this for governance-oriented review packets.',
            ],
            codeSample: `Prompt:
Create a quarterly access review summary with provider coverage, user/grant totals, high-risk findings, unresolved gaps, and prioritized remediation actions. Keep this read-only.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: produce a concise executive summary plus reviewer-ready details and a clear remediation queue with owners.',
                },
            ],
        },
        {
            id: 'pattern-incident-summary',
            title: 'Pattern 6: Incident Evidence Output',
            tocTitle: 'Pattern 6: Incident',
            headingLevel: 3,
            paragraphs: [
                'Use this for time-bounded incident response support.',
            ],
            codeSample: `Prompt:
For incident window <start> to <end>, collect read-only access and audit evidence. Return an actor timeline, affected identities, privilege changes, and confidence notes where data is missing.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: constrain collection to the incident window, produce a timeline with confidence annotations, and separate confirmed evidence from assumptions.',
                },
            ],
        },
        {
            id: 'report-schema',
            title: 'Recommended Output Schema',
            tocTitle: 'Output Schema',
            paragraphs: [
                'Require this structure in every run output for consistency:',
                '1) `run_scope` (quarterly or incident, time window, providers requested).',
                '2) `provider_coverage` (success/fail/partial + reason).',
                '3) `identity_inventory` (users, groups/roles, key counts).',
                '4) `permission_posture` (grants/policies, high-risk findings).',
                '5) `audit_evidence` (events collected + notable actions).',
                '6) `collection_gaps` (endpoint limitations, blocked data, or unavailable provider responses).',
                '7) `recommended_next_actions` (prioritized remediation and owners).',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Keep raw secrets and token-like values redacted in both terminal output and exported summaries.',
                },
            ],
        },
        {
            id: 'safety',
            title: 'Safety and Review Controls',
            tocTitle: 'Safety',
            paragraphs: [
                'Run this playbook in read-only mode by default and reject any prompt that implies mutation unless explicitly approved.',
                'If any provider preflight fails because of endpoint/scope limitations, continue with partial collection only when requested and always include a remediation plan.',
                'Use workflow and tool-call review in the TUI while the agent is connected to monitor actions in real time.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'For manual review workflows and run approval practices, see `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            tocTitle: 'Next Steps',
            paragraphs: [
                'If you need additional provider coverage (for example, GitHub or AWS CloudTrail), extend catalogs and add matching read-only collection steps.',
                'Export your finalized collection workflows and reporting templates into repository docs for repeat quarterly execution.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Import and tune catalogs with `/docs/learn/library-and-catalogs` before adding new provider goals.',
                },
            ],
        },
    ],
};
