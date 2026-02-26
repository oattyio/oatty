import type {DocsPage} from '../types';

/**
 * Credential rotation readiness playbook page model.
 *
 * This guide helps teams audit key/token hygiene with read-only evidence
 * before planning or executing any rotation changes.
 */
export const credentialRotationReadinessPlaybookPage: DocsPage = {
    path: '/docs/guides/credential-rotation-readiness-playbook',
    title: 'Credential Rotation Readiness Playbook',
    summary: 'A prompt-driven, read-only guide to assess token/key age, usage, and rotation risk across GitHub, AWS IAM, and Datadog before any write actions.',
    learnBullets: [
        'If required catalogs are missing, ask the agent to import them and verify auth headers first.',
        'Run read-only credential inventory and usage checks by provider.',
        'Identify stale, unused, or high-risk credentials and map blast radius.',
        'Generate a phased rotation plan with explicit verification gates.',
        'Separate readiness assessment from mutation steps.',
    ],
    estimatedTime: '12-20 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'purpose',
            title: 'Purpose',
            tocTitle: 'Purpose',
            paragraphs: [
                'Use this playbook to determine if your environment is ready for safe credential rotation without making any changes.',
                'If required provider catalogs or mandatory auth headers are not yet configured, begin with an import-and-verify step before running readiness prompts.',
                'Use it for quarterly security hygiene reviews, pre-rotation planning, and incident-driven key compromise response.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'For manual header verification or updates, use `/docs/learn/library-and-catalogs#headers-management`.',
                },
                {
                    type: 'tip',
                    content: 'For execution safeguards and approval flow, use `/docs/learn/how-oatty-executes-safely` and `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'validated-read-only-signals',
            title: 'Validated Read-Only Signals',
            tocTitle: 'Validated Signals',
            paragraphs: [
                'Representative rotation-readiness signals available through Oatty command catalogs:',
                'Datadog API keys: `datadog api_keys:list` (includes last-used metadata).',
                'Datadog application keys: `datadog current_user:application_keys:list` (includes last-used timestamps).',
                'AWS IAM credential report: `aws #Action=GetCredentialReport:list` (account-wide credential age/status, when auth is configured).',
                'AWS IAM access keys: `aws #Action=ListAccessKeys:list` (principal key inventory, when auth is configured).',
                'GitHub SSH/GPG keys: `github user:keys:list`, `github user:gpg_keys:list` (requires authenticated user context).',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'These read-only signals are sufficient to produce a practical readiness report and a phased rotation plan before any write actions.',
                },
            ],
        },
        {
            id: 'how-to-use',
            title: 'How to Use This Guide (Goal-Oriented, NL-First)',
            tocTitle: 'How to Use',
            paragraphs: [
                'Describe the rotation goal, systems in scope, and time constraints. Let the agent discover exact commands for your environment.',
                'If provider catalogs are missing, have the agent import them first and verify required headers before collecting evidence.',
                'Keep this phase read-only: inventory credentials, map usage and owners, and highlight rotation blockers.',
                'Require outputs in a normalized schema so readiness can be compared across runs.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If you need a command-discovery refresher, see `/docs/learn/search-and-run-commands`.',
                },
            ],
        },
        {
            id: 'prompt-patterns-overview',
            title: 'Prompt Patterns: Prompt -> Expected Agent Actions',
            tocTitle: 'Prompt Patterns',
            headingLevel: 2,
            paragraphs: [
                'Use these patterns to run a consistent readiness assessment before rotating credentials.',
            ],
        },
        {
            id: 'pattern-1-inventory',
            title: 'Pattern 1: Credential Inventory (Read-Only)',
            tocTitle: 'Pattern 1: Inventory',
            headingLevel: 3,
            paragraphs: [
                'Start by listing all relevant credential objects and their owners.',
            ],
            codeSample: `Prompt:
Run a read-only credential inventory for GitHub, AWS IAM, and Datadog. If a required catalog is missing, import it first and verify required headers. Return keys/tokens by owner with creation time, last-used time, and status.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: import missing catalogs when needed, verify required headers, collect credential inventories per provider, normalize fields, and return counts plus owner mapping.',
                },
            ],
        },
        {
            id: 'pattern-2-risk-classification',
            title: 'Pattern 2: Risk Classification',
            tocTitle: 'Pattern 2: Risk',
            headingLevel: 3,
            paragraphs: [
                'Classify stale or risky credentials before proposing rotation sequence.',
            ],
            codeSample: `Prompt:
Classify credentials into risk tiers (unused, stale, high privilege, unknown owner). Show evidence for each classification and keep this read-only.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: apply risk rules to inventory signals, flag high-priority credentials, and include confidence notes where data is incomplete.',
                },
            ],
        },
        {
            id: 'pattern-3-blast-radius',
            title: 'Pattern 3: Blast Radius and Dependency Mapping',
            tocTitle: 'Pattern 3: Blast Radius',
            headingLevel: 3,
            paragraphs: [
                'Map operational impact before any key/token change is attempted.',
            ],
            codeSample: `Prompt:
For each high-risk credential, estimate blast radius: dependent services, automation paths, and likely failure impact if revoked. Keep it evidence-based and read-only.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: map dependencies and affected systems, identify unknown ownership, and separate confirmed dependencies from inferred ones.',
                },
            ],
        },
        {
            id: 'pattern-4-rotation-plan',
            title: 'Pattern 4: Phased Rotation Plan',
            tocTitle: 'Pattern 4: Plan',
            headingLevel: 3,
            paragraphs: [
                'Generate a no-surprises plan with verification and rollback gates.',
            ],
            codeSample: `Prompt:
Draft a phased rotation plan (prepare, rotate, verify, revoke-old) with approval gates, rollback steps, and success criteria for each provider. Do not execute changes.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: produce an ordered, low-risk rotation plan with per-phase checks and a clear operator approval model.',
                },
            ],
        },
        {
            id: 'pattern-5-executive-summary',
            title: 'Pattern 5: Read-Only Executive Summary',
            tocTitle: 'Pattern 5: Summary',
            headingLevel: 3,
            paragraphs: [
                'Use this for leadership review before scheduling rotation windows.',
            ],
            codeSample: `Prompt:
Provide an executive readiness summary: current risk posture, top blockers, phased rotation recommendation, and unresolved unknowns. Keep this read-only.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: summarize key risks, readiness level, and immediate next actions with owners and priority.',
                },
            ],
        },
        {
            id: 'output-schema',
            title: 'Recommended Output Schema',
            tocTitle: 'Output Schema',
            paragraphs: [
                'Use a fixed structure for auditability and comparison over time:',
                '1) `scope` (providers, credential types, time window).',
                '2) `credential_inventory` (object, owner, age, last_used, status).',
                '3) `risk_classification` (tier, reason, confidence).',
                '4) `blast_radius` (dependencies, impact, unknowns).',
                '5) `rotation_plan` (phases, gates, rollback).',
                '6) `open_questions` (missing evidence and owner follow-ups).',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Redact sensitive values and include only identifiers needed for operator action.',
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            tocTitle: 'Next Steps',
            paragraphs: [
                'After readiness is approved, run rotation in a separate write-enabled workflow with explicit pause points.',
                'Export readiness outputs and plan artifacts into repository docs for quarterly evidence and change review.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Use `/docs/learn/workflows-basics` to review and run staged workflows with clear approval gates.',
                },
            ],
        },
    ],
};
