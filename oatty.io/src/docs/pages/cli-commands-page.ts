import type {DocsPage} from '../types';

/**
 * CLI command reference page model.
 *
 * This page is a lookup reference for CLI invocation patterns, command discovery,
 * and execution routing behavior.
 */
export const cliCommandsPage: DocsPage = {
    path: '/docs/reference/cli-commands',
    title: 'CLI Command Reference',
    summary: 'Use this page as a command lookup for discovery, inspection, and execution flows in Oatty CLI mode.',
    learnBullets: [
        'Understand canonical command identifiers and CLI argument shape.',
        'Discover commands with predictable search patterns.',
        'Inspect command schemas before execution.',
        'Route execution by HTTP method and safety guarantees.',
    ],
    estimatedTime: '8-12 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'canonical-identifiers',
            title: 'Canonical Command Identifiers',
            paragraphs: [
                'Oatty resolves commands in canonical `<group> <command>` form.',
                'Use canonical identifiers when inspecting or running commands to avoid ambiguity.',
                'Treat canonical IDs as stable references for scripts and workflow steps.',
            ],
            codeSample: `# General pattern
oatty <group> <command> [flags]

# Example
oatty apps apps:list --project-id proj_123`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Canonical IDs remain consistent across discovery and execution paths.'
                },
                {
                    type: 'recovery',
                    content: 'If you only have vendor CLI syntax, run command search first and copy the canonical ID from results.'
                },
            ],
        },
        {
            id: 'command-discovery',
            title: 'Command Discovery',
            paragraphs: [
                'Use search first when you do not know the exact command path.',
                'Keep search terms short and task-oriented, then narrow with specific nouns.',
                'After selecting a candidate, inspect schema before execution.',
            ],
            codeSample: `# Fuzzy discovery
oatty search "project domain"

# Narrow intent
oatty search "projects list"

# Review root help for available command groups
oatty --help`,
            callouts: [
                {
                    type: 'tip',
                    content: 'Prefer discovery output over memorized commands when catalogs change.'
                },
                {
                    type: 'advanced',
                    content: 'For repeated automation, validate once in TUI/CLI, then pin the exact command line in scripts.'
                },
            ],
        },
        {
            id: 'schema-inspection',
            title: 'Schema Inspection and Input Review',
            paragraphs: [
                'Review command details before sending requests to production systems.',
                'Confirm required positional arguments, required flags, and payload shape.',
                'Use help output to compare expected input names with your script variables.',
            ],
            codeSample: `# Inspect one command in detail
oatty help apps apps:create

# Alternate: contextual help during TUI command selection
# press F1 in Run Command`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Help output should identify required and optional command inputs.'
                },
                {
                    type: 'recovery',
                    content: 'If execution fails due to missing inputs, map each error field to the corresponding required flag or positional argument.'
                },
            ],
        },
        {
            id: 'execution-routing',
            title: 'Execution Routing and Safety Model',
            paragraphs: [
                'Execution mode follows command backing type and HTTP method.',
                'Read-only requests are routed through safe execution paths.',
                'Mutating requests are routed through non-destructive or destructive paths depending on method.',
            ],
            codeSample: `# Read-only (HTTP GET)
run_safe_command

# Non-destructive write (HTTP POST/PUT/PATCH)
run_command

# Destructive write (HTTP DELETE)
run_destructive_command`,
            callouts: [
                {
                    type: 'tip',
                    content: 'Use preview/inspection before destructive operations.'
                },
                {
                    type: 'advanced',
                    content: 'In workflows, keep destructive steps isolated and clearly labeled for easier review and rollback planning.'
                },
            ],
        },
        {
            id: 'automation-patterns',
            title: 'Automation Patterns',
            paragraphs: [
                'Use CLI mode for deterministic non-interactive runs in CI/CD or scheduled jobs.',
                'Keep inputs explicit and environment-driven where possible.',
                'Capture stdout/stderr in your job logs for auditability and failure triage.',
            ],
            codeSample: `# Script-friendly command
oatty workflow run deploy --input env=staging

# Standard shell guard pattern
set -euo pipefail
oatty search "apps list"`,
            callouts: [
                {
                    type: 'fallback',
                    content: 'When TUI discovery identifies the right command, copy that exact command into scripts instead of rewriting it from memory.'
                },
                {
                    type: 'recovery',
                    content: 'If CI runs behave differently, verify catalog availability, headers, and environment variables in the job context.'
                },
            ],
        },
    ],
};
