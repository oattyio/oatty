import type {DocsPage} from '../types';

/**
 * Trust-first execution model page.
 *
 * This page explains how Oatty keeps operator control while still benefiting from
 * natural language assistance and automation.
 */
export const howOattyExecutesSafelyPage: DocsPage = {
    path: '/docs/learn/how-oatty-executes-safely',
    title: 'How Oatty Executes Safely',
    summary: 'Understand the trust model: suggestion, preview, validation, and explicit operator control before execution.',
    learnBullets: [
        'Connect Oatty MCP tooling to your AI assistant for controlled planning support.',
        'Separate suggestion from execution so generated plans stay reviewable.',
        'Use preview and validation outputs before running commands or workflows.',
        'Interpret failures quickly and recover with deterministic next steps.',
        'Keep manual control even when using AI assistants and natural language requests.',
    ],
    estimatedTime: '6-9 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'trust-model',
            title: 'Trust Model at a Glance',
            paragraphs: [
                'Connect Oatty to your AI assistant through MCP so planning and execution tools are discoverable in one place.',
                'Oatty treats natural language as a planning input, not an execution bypass.',
                'In the TUI, suggested commands and workflows are reviewable before execution. Connected AI assistants can run/debug workflows if desired.',
                'Execution remains explicit and observable through status, logs, and result views.',
            ],
            callouts: [
                {type: 'tip', content: 'Use this mental model: suggest -> inspect -> validate -> run.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-run.png',
                    imageAlt: 'Run command view with output and logs',
                    content: 'Capture a run view that shows selected command, output, and logs together.',
                },
            ],
        },
        {
            id: 'preview-validation',
            title: 'Preview and Validation Before Run',
            paragraphs: [
                'Before execution, confirm command arguments or workflow inputs match intent.',
                'Use preview and validation tools to catch schema, dependency, input, and command/catalog preflight issues early.',
                'When validation fails, use returned violations and suggested actions to repair quickly.',
            ],
            callouts: [
                {type: 'expected', content: 'Validation failures are specific enough to fix in one edit cycle.'},
                {
                    type: 'recovery',
                    content: 'If a step fails validation, correct the reported field/dependency and run validation again before execution.'
                },
                {
                    type: 'advanced',
                    content: 'For workflows, use `workflow.resolve_inputs` for input/provider readiness, then `workflow.validate` (or run precheck) for command/catalog readiness before execution.'
                },
            ],
        },
        {
            id: 'operator-control',
            title: 'Operator Control and Manual Overrides',
            paragraphs: [
                'Assisted planning does not remove manual operation paths.',
                'You can still run commands directly, edit workflow manifests, and provide inputs manually.',
                'This keeps behavior deterministic and audit-friendly in high-stakes changes.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'For production changes, prefer explicit command/workflow review over one-shot execution.'
                },
                {
                    type: 'fallback',
                    content: 'Use CLI commands for explicit scriptable execution when you need non-interactive control.'
                },
            ],
        },
        {
            id: 'failure-recovery',
            title: 'Failure and Recovery Pattern',
            paragraphs: [
                'Treat failures as structured feedback, not dead ends.',
                'Read the first actionable error, update the relevant input/spec, and rerun the smallest validation step first.',
                'Then rerun execution once readiness checks pass.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Most failures should map to a clear next action from validation or error metadata.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/docs-screenshot-placeholder.svg',
                    imageAlt: 'Failure and recovery screenshot placeholder',
                    content: 'Capture a single recovery flow: failure signal -> correction -> successful rerun.',
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: [
                'Continue to Search and Run Commands for command-level execution patterns.',
                'Continue to Workflows Basics for input collection, run controls, and step-level inspection.',
                'Use workflow export/import flows to share reviewed workflows with teammates and CI pipelines.',
            ],
            callouts: [{
                type: 'expected',
                content: 'You can evaluate assisted suggestions without sacrificing operator control.'
            }],
        },
    ],
};
