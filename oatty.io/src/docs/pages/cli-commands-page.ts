import type {DocsPage} from '../types';

/**
 * CLI command reference page model.
 *
 * This page is a lookup reference for CLI invocation patterns, command discovery,
 * and direct execution behavior.
 */
export const cliCommandsPage: DocsPage = {
    path: '/docs/reference/cli-commands',
    title: 'CLI Command Reference',
    summary: 'Use this page as a command lookup for discovery, inspection, and execution flows in Oatty CLI mode.',
    learnBullets: [
        'Understand canonical command identifiers and CLI argument shape.',
        'Discover commands through the supported TUI-first workflow.',
        'Inspect command schemas before execution.',
        'Run workflows and exact command paths from the terminal.',
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
                    content: 'If you only have vendor CLI syntax, use the TUI to discover the matching Oatty command path first, then copy the exact canonical ID into your shell command.'
                },
            ],
        },
        {
            id: 'command-discovery',
            title: 'Command Discovery',
            paragraphs: [
                'Use the TUI when you do not know the exact command path yet.',
                'After you import a catalog, rerun `oatty --help` or inspect command-specific help to confirm which top-level groups are available.',
                'Once you know the exact canonical path, execute that command directly from the CLI.',
            ],
            codeSample: `# Review built-in and imported top-level commands
oatty --help

# Inspect one imported command once you know the path
oatty <group> <command> --help`,
            callouts: [
                {
                    type: 'tip',
                    content: 'Prefer TUI discovery over memorized commands when catalogs change.'
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
oatty <group> <command> --help

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
            id: 'run-exact-commands',
            title: 'Run Exact Commands and Workflows',
            paragraphs: [
                'Run exact catalog-backed commands once you know the canonical path.',
                'Use workflow subcommands for repeatable automation that lives in workflow files or imported workflow IDs.',
                'Keep command inputs explicit so terminal runs are easy to review and reuse.',
            ],
            codeSample: `# Run an imported command once the path is known
oatty <group> <command> [flags]

# Preview and run workflows
oatty workflow preview --file ./workflow.yaml
oatty workflow run --file ./workflow.yaml --input env=staging`,
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
oatty workflow run --file ./workflow.yaml --input env=staging

# Standard shell guard pattern
set -euo pipefail
oatty <group> <command> [flags]`,
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
