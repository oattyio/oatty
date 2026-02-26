import type {DocsPage} from '../types';

/**
 * Vercel -> Render migration playbook page model.
 *
 * This guide captures a realistic user-agent exchange and turns it into
 * a reviewable migration workflow for app + Postgres cutover.
 */
export const vercelToRenderMigrationPlaybookPage: DocsPage = {
    path: '/docs/guides/vercel-to-render-migration-playbook',
    title: 'Vercel -> Render Migration Playbook (Reuse Existing Postgres URL)',
    summary: 'A prompt-driven, workflow-aligned guide for migrating a Vercel app to Render while reusing the existing Postgres `DATABASE_URL`.',
    learnBullets: [
        'Drive migration with prompts and explicit expected agent actions.',
        'Inventory Vercel + Render state before running writes.',
        'Create or reuse a Render web service and set `DATABASE_URL` safely.',
        'Validate outcomes with optional restart and post-run checks.',
    ],
    estimatedTime: '12-20 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'purpose',
            title: 'Purpose',
            tocTitle: 'Purpose',
            paragraphs: [
                'This guide uses the `vercel_to_render_neon` workflow name in prompts and examples.',
                'The workflow migrates app runtime from Vercel to Render while reusing an existing Postgres connection string (`DATABASE_URL`), rather than provisioning or migrating databases.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You get a concrete, repeatable path: discover project state, create/find Render service, upsert `DATABASE_URL`, and optionally restart.',
                },
            ],
        },
        {
            id: 'workflow-scope',
            title: 'Workflow Scope',
            tocTitle: 'Workflow Scope',
            paragraphs: [
                'Current in-scope actions for `vercel_to_render_neon`:',
                '1) Read Vercel project details and locate `DATABASE_URL` (or use an explicit override).',
                '2) Find an existing Render web service by name, or create one if missing.',
                '3) Upsert `DATABASE_URL` on the target Render service.',
                '4) Optionally restart the Render service after env var update.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Out of scope for this workflow: DNS/domain cutover, cross-database data migration, and blue/green traffic switching.',
                },
            ],
        },
        {
            id: 'how-to-use-this-guide',
            title: 'How to Use This Guide (Goal-Oriented, NL-First)',
            tocTitle: 'How to Use',
            paragraphs: [
                'This guide is outcome-oriented: use natural-language prompts to communicate migration goals, constraints, and gates, then evaluate whether agent actions satisfy those goals.',
                'For this workflow, step execution is deterministic once inputs are fixed, but input collection and environment mapping are non-deterministic and should be handled through explicit prompt-and-review loops.',
                'Keep safety invariants stable: do read-only discovery first, review conditional execution paths before write steps, redact secrets in reports, and finish with pass/fail evidence.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can use the same goal prompts across environments even when resource IDs, names, or surrounding deployment details differ.',
                },
            ],
        },
        {
            id: 'prompt-patterns-overview',
            title: 'Prompt Patterns: Prompt -> Expected Agent Actions',
            tocTitle: 'Prompt Patterns',
            headingLevel: 2,
            paragraphs: [
                'Use these prompts as realistic operator commands for a connected agent. Each prompt maps to expected actions aligned to the workflow behavior.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If IDs differ in your environment, ask the agent to discover available commands/workflows first and substitute exact IDs before execution.',
                },
            ],
        },
        {
            id: 'pattern-1-inventory',
            title: 'Pattern 1: Inventory and Input Collection (Read-Only)',
            tocTitle: 'Pattern 1: Inventory',
            headingLevel: 3,
            paragraphs: [
                'Start with discovery to gather workflow inputs and verify prerequisites.',
            ],
            codeSample: `Prompt:
Inventory Vercel project and Render workspace state for this migration. Do not write anything. Return required workflow inputs and identify any missing values.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: discover project/service state, resolve owner/project identifiers, and report missing required inputs before execution.',
                },
                {
                    type: 'fallback',
                    content: 'Representative discovery commands: `vercel projects:list`, `vercel projects:info`, `vercel projects:env:list`, `render owners:list`, `render services:list`.',
                },
            ],
        },
        {
            id: 'pattern-2-prepare-run',
            title: 'Pattern 2: Prepare a Safe Run Plan',
            tocTitle: 'Pattern 2: Plan',
            headingLevel: 3,
            paragraphs: [
                'Use the agent to preview what will happen for your inputs before running writes.',
            ],
            codeSample: `Prompt:
Prepare a run plan for vercel_to_render_neon using my inputs. Show which steps will execute, which are conditional, and any risks before I approve.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: resolve conditions (existing service vs new service, Vercel value vs override), surface risks, and request approval before writes.',
                },
            ],
        },
        {
            id: 'pattern-3-run-migration',
            title: 'Pattern 3: Run Migration Workflow',
            tocTitle: 'Pattern 3: Execute',
            headingLevel: 3,
            paragraphs: [
                'Execute the migration workflow after review and approval.',
            ],
            codeSample: `Prompt:
Run vercel_to_render_neon now with the approved inputs. Reuse existing Render service if present; create it only if missing. Then set DATABASE_URL and restart if enabled.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: execute workflow steps in order, report which branch was taken (existing/new service, override/derived URL), and return resulting service identifiers.',
                },
                {
                    type: 'fallback',
                    content: 'Representative mutating commands: `render services:create`, `render services:env-vars:add-or-update-environment-variable`, `render services:restart:create`.',
                },
            ],
        },
        {
            id: 'pattern-4-post-run-validation',
            title: 'Pattern 4: Post-Run Validation',
            tocTitle: 'Pattern 4: Validate',
            headingLevel: 3,
            paragraphs: [
                'Validate that app runtime and environment configuration are correct after the workflow completes.',
            ],
            codeSample: `Prompt:
Validate migration outcomes without further writes. Confirm the Render service exists, DATABASE_URL is set, and restart status is healthy.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: run read-oriented checks, summarize deployment health, and report pass/fail for each acceptance item.',
                },
                {
                    type: 'tip',
                    content: 'Keep `DATABASE_URL` values redacted in summaries and logs.',
                },
            ],
        },
        {
            id: 'pattern-5-read-only-summary',
            title: 'Pattern 5: Read-Only Executive Summary',
            tocTitle: 'Pattern 5: Summary',
            headingLevel: 3,
            paragraphs: [
                'Use a read-only summary when reporting status to stakeholders.',
            ],
            codeSample: `Prompt:
Produce an executive summary of this Vercel-to-Render migration run with completed steps, unresolved risks, and next actions. Do not perform any writes.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: summarize outcomes from completed workflow steps, list open items, and avoid mutating operations.',
                },
            ],
        },
        {
            id: 'workflow-blueprint',
            title: 'Workflow Blueprint (Recommended Run Order)',
            tocTitle: 'Run Order',
            paragraphs: [
                'Use this run order for predictable, reviewable execution:',
                '1) Inventory and inputs -> 2) Review conditional run plan -> 3) Execute `vercel_to_render_neon` -> 4) Post-run validation -> 5) Executive summary.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Use the Workflows view to review/approve generated workflows and inspect step-level outcomes: `/docs/learn/workflows-basics`.',
                },
            ],
        },
        {
            id: 'rollback',
            title: 'Rollback and Safety Notes',
            tocTitle: 'Rollback',
            paragraphs: [
                'For this workflow scope, rollback is configuration-focused:',
                'Re-point runtime traffic to the prior Vercel deployment if Render validation fails.',
                'Restore prior `DATABASE_URL` value on Render (or remove it) if incorrect.',
                'Disable optional restart behavior during retries if you need staged verification first.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can revert app runtime behavior quickly without performing cross-database rollback operations.',
                },
            ],
        },
        {
            id: 'acceptance-checklist',
            title: 'Migration Acceptance Checklist',
            tocTitle: 'Checklist',
            paragraphs: [
                'Render service is healthy and serving expected responses.',
                '`DATABASE_URL` is present on Render service and points to the intended database target.',
                'Workflow branch outcome is documented (existing vs new service, override vs Vercel-derived URL).',
                'If restart was enabled, restart completed successfully.',
                'No secret values were exposed in logs or reports.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Migration is complete for this workflow when runtime and environment checks pass and residual risk is accepted.',
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            tocTitle: 'Next Steps',
            paragraphs: [
                'If you need full domain cutover or cross-database migration, add separate workflows for those phases and keep them behind explicit approvals.',
                'Export reviewed workflow runs and operator notes into repository docs for auditability.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Use a periodic read-only validation prompt to detect config drift after migration.',
                },
            ],
        },
        {
            id: 'extension-workflows-overview',
            title: 'Extension Goals (For Full Migration Coverage)',
            tocTitle: 'Extension Goals',
            paragraphs: [
                'Use these extension workflows for phases intentionally out of scope for `vercel_to_render_neon`.',
                'Each extension should be implemented as its own workflow with explicit inputs, gate criteria, and acceptance checks.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'Before running extension prompts, ask the agent to list available workflows and use the exact workflow IDs it finds in your environment.',
                },
                {
                    type: 'tip',
                    content: 'Manual operator control points are easiest to run from `/docs/learn/workflows-basics` with guardrails from `/docs/learn/how-oatty-executes-safely`.',
                },
            ],
        },
        {
            id: 'extension-domain-cutover',
            title: 'Extension Goal 1: Domain Cutover',
            tocTitle: 'Extension 1: Domains',
            headingLevel: 3,
            paragraphs: [
                'Use a dedicated domain-cutover workflow for this goal. If one does not exist yet, ask the agent to draft it before proceeding.',
                'Use this extension after Render runtime validation is green and before final traffic switch.',
            ],
            codeSample: `Prompt:
Run the domain cutover extension workflow for this migration. Pause before final DNS changes, show me the exact records to change, and continue only after explicit approval.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: verify Render domain readiness, propose DNS changes, pause for approval, execute switch, and run post-switch domain/TLS checks.',
                },
                {
                    type: 'tip',
                    content: 'Gate criteria: Render service health is green, rollback target is confirmed, and DNS change window is approved.',
                },
            ],
        },
        {
            id: 'extension-database-migration',
            title: 'Extension Goal 2: Database Migration',
            tocTitle: 'Extension 2: Database',
            headingLevel: 3,
            paragraphs: [
                'Use a dedicated database-migration workflow for this goal. If one does not exist yet, ask the agent to draft it before proceeding.',
                'Use this extension only when moving data to a new database target; skip if you continue reusing the existing `DATABASE_URL`.',
            ],
            codeSample: `Prompt:
Run the database migration extension workflow with pre-migration backup confirmation, parity checks, and rollback notes. Stop immediately if any integrity check fails.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: verify source/target connectivity, enforce backup gate, execute migration sequence, run schema/row-count parity checks, and report pass/fail with rollback readiness.',
                },
                {
                    type: 'advanced',
                    content: 'Gate criteria: backup/snapshot exists, migration window is active, validation queries are defined, and operator approval is recorded.',
                },
            ],
        },
        {
            id: 'extension-post-cutover-audit',
            title: 'Extension Goal 3: Post-Cutover Audit',
            tocTitle: 'Extension 3: Audit',
            headingLevel: 3,
            paragraphs: [
                'Use a dedicated post-cutover audit workflow for this goal. If one does not exist yet, ask the agent to draft it before proceeding.',
                'Use this extension after runtime/domain cutover to produce an operator and leadership-ready status report.',
            ],
            codeSample: `Prompt:
Run the post-cutover audit extension workflow in read-only mode and return an executive pass/fail summary with unresolved risks and immediate next actions.`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Expected agent actions: execute read-only checks for health, config integrity, and recent deploy state; then emit concise status, risks, and recommended follow-up actions.',
                },
                {
                    type: 'tip',
                    content: 'Gate criteria: cutover is complete, monitoring data is available, and no in-progress rollback is active.',
                },
            ],
        },
    ],
};
