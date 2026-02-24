import type {DocsPage} from '../types';

/**
 * Search and Run Commands page model.
 *
 * This page teaches the core command discovery and execution loop in the TUI.
 */
export const searchAndRunCommandsPage: DocsPage = {
    path: '/docs/learn/search-and-run-commands',
    title: 'Search for and Run Commands',
    summary: 'Use the TUI command flow to find commands quickly, execute with confidence, and inspect results without after completion.',
    learnBullets: [
        'Run the primary TUI search-to-execution path.',
        'Use command help and hints before execution.',
        'Use Find browser to inspect and hand off commands.',
        'Use CLI fallback for automation and scripts.',
    ],
    estimatedTime: '10-14 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'flow',
            title: 'Overview',
            paragraphs: [
                'Use the TUI to find and execute commands.',
                'Use command help and hints to confirm expected inputs and command behavior.',
                'Leverage value providers to provide dynamic inputs.',
                'Use CLI fallback for automation and scripts.',
            ],
            callouts: [
                {
                    type: 'expected',
                    label: 'Goal',
                    content: 'You can find and execute commands in the TUI with minimal friction.'
                },
                {
                    type: 'screenshot',
                    label: 'Run Command Flow',
                    imageSrc: '/Oatty-command-run-flow.webp',
                    imageAlt: 'Run command flow',
                    content: 'Complete run flow that includes selected command, output, dynamic input and logs.'
                },
            ]
        },
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            paragraphs: ['Launch Oatty with `oatty`.', 'Import at least one catalog so commands are discoverable.', 'Keep logs available for execution verification.'],
            codeSample: `oatty`,
            callouts: [
                {
                    type: 'expected',
                    content: 'You can open Run Command and see command suggestions from your imported catalog.'
                },
                {type: 'recovery', content: 'If no commands appear, import a catalog first in Library.'},
                {
                    type: 'screenshot',
                    label: 'Run Command Empty Input',
                    imageSrc: '/Oatty-run-empty.png',
                    imageAlt: 'Run command screenshot',
                    content: 'Capture Run Command focused with an empty input and visible hints.'
                },
                {type: 'fallback', content: 'Catalog import fallback: `oatty import <path-or-url> --kind catalog`.'},
            ],
        },
        {
            id: 'search-and-select',
            title: 'Search for and Select a Command',
            paragraphs: ['Navigate to Run Command from the left navigation.', 'Type a task phrase such as `create`, then press TAB.', 'Use Up and Down to change selection in the suggestion list.'],
            callouts: [
                {type: 'expected', content: 'Relevant commands are listed and selectable in the suggestion list.'},
                {
                    type: 'recovery',
                    content: 'If search returns nothing, verify catalog import and try broader search terms.'
                },
                {
                    type: 'screenshot',
                    label: 'Run Command Suggestions',
                    imageSrc: '/Oatty-command-suggestions.png',
                    imageAlt: 'Run command screenshot',
                    content: 'Suggestion list open with one highlighted command.'
                },
                {
                    type: 'advanced',
                    content: 'Suggestions are positional. Command arguments and flags are also searchable once a command is selected from the list.'
                },
            ],
        },
        {
            id: 'review-help',
            title: 'Review Help Before Running',
            paragraphs: ['Open command help from the active command context by pressing F1.', 'Verify required inputs and expected command shape.', 'Return to input using Esc the key and complete required values.'],
            callouts: [
                {type: 'expected', content: 'Help modal shows the full command metadata.'},
                {
                    type: 'recovery',
                    content: 'If help is not shown, verify the desired command is selected in the suggestions list or the command has been typed in the input.'
                },
                {
                    type: 'screenshot',
                    label: 'Review Command Help',
                    imageSrc: '/Oatty-review-command-help.png',
                    imageAlt: 'Oatty help modal screenshot',
                    content: 'Command help visible with required input details.'
                },
                {
                    type: 'advanced',
                    content: 'Use help to quickly reference the full command details including args, flags, and defaults. Use Esc to close help and return to input.'
                },
            ],
        },
        {
            id: 'execute-command',
            title: 'Execute and Inspect Output',
            paragraphs: ['Run the selected command from the command runner.', 'Inspect structured output in the result view.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Execution reaches a terminal status, and output/logs show the final result.'
                },
                {
                    type: 'recovery',
                    content: 'If execution fails, an error message is displayed and a log entry made - adjust inputs or address message contents and rerun.'
                },
                {
                    type: 'screenshot',
                    label: 'Run Command Result',
                    imageSrc: '/Oatty-command-results.png',
                    imageAlt: 'Run command screenshot',
                    content: 'Result table showing the payload af the command.'
                },
                {
                    type: 'fallback',
                    content: 'Run the same command in CLI for scripts and CI with explicit flags and arguments.'
                },
            ],
        },
        {
            id: 'find-browser-handoff',
            title: 'Use Find/Browser for Discovery',
            paragraphs: ['Open Find to browse commands with summaries and categories.', 'Select a command and send it to Run Command.', 'Execute from Run Command after reviewing inputs.'],
            callouts: [
                {type: 'expected', content: 'A command selected in Find appears in Run Command ready for execution.'},
                {
                    type: 'recovery',
                    content: 'If handoff does not occur, confirm focus is in Find and retry the handoff action shown in hints.'
                },
                {
                    type: 'screenshot',
                    label: 'Find and Run Command Flow',
                    imageSrc: '/Oatty-find-and-runcommand-flow.webp',
                    imageAlt: 'Oatty find and run command flow',
                    content: 'Find command then run it'
                },
                {
                    type: 'advanced',
                    content: 'Find is best for exploration; Run Command is optimized for fast execution loops.'
                },
            ],
        },
        {
            id: 'cli-fallback',
            title: 'CLI Fallback for Automation',
            paragraphs: ['Use CLI search when you need non-interactive discovery.', 'Run commands directly in scripts and CI with explicit inputs.'],
            callouts: [
                {type: 'expected', content: 'You can execute the same command path outside the TUI.'},
                {
                    type: 'recovery',
                    content: 'If CLI execution differs from TUI expectation, verify command arguments and active catalog configuration.'
                },
                {
                    type: 'advanced',
                    content: 'Use TUI for discovery and validation first, then promote stable command lines into automation.'
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: ['Continue to Library and Catalogs to manage command sources.', 'Then continue to Workflows to compose repeatable multi-step execution.'],
            callouts: [
                {type: 'expected', content: 'You can discover and run commands reliably in both TUI and CLI contexts.'},
            ],
        },
    ],
};
