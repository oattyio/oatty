import type {DocsPage} from '../types';

/**
 * Workflows Basics page model.
 *
 * This page documents the standard workflow lifecycle in the TUI.
 */
export const workflowsBasicsPage: DocsPage = {
    path: '/docs/learn/workflows-basics',
    title: 'Workflows Basics',
    summary: 'Move from workflow selection to input collection to execution, then control active runs from the run view.',
    learnBullets: [
        'Import and remove workflows from the workflow list.',
        'Open pre-run inputs and resolve required values.',
        'Run workflows and inspect step status and details.',
        'Use pause/resume/cancel controls during active runs.',
    ],
    estimatedTime: '12-16 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'overview',
            title: 'Overview',
            paragraphs: [
                'Workflows are defined in YAML files and imported into Oatty via the CLI or MCP HTTP server.',
                'Workflows are executed in the TUI using the Workflows Runner.',
                'Workflows can be imported from local files or URLs, and can be managed via the CLI or MCP HTTP server.',
            ],
            callouts: [
                {
                    type: 'expected',
                    label: 'Goal',
                    content: 'Use the Workflows Runner to import and execute workflows, then inspect the results.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-workflow-flow.webp',
                    imageAlt: 'Workflow execution screenshot',
                    content: 'Capture import action and remove confirmation flow.'
                },
            ]
        },
        {
            id: 'manage-list',
            title: 'Manage Workflow List',
            paragraphs: ['Use Import to add workflows to the list.', 'Select a workflow and use Remove when needed.', 'Use search and list navigation to locate workflows quickly.'],
            callouts: [
                {type: 'expected', content: 'Workflow list reflects import/remove actions and selection state.'},
                {type: 'recovery', content: 'If Remove is unavailable, select a workflow first.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-import-workflow-flow.webp',
                    imageAlt: 'Workflow execution screenshot',
                    content: 'Capture import action and remove confirmation flow.'
                },
                {
                    type: 'advanced',
                    content: 'List navigation supports row movement and page jumps for larger workflow sets.'
                },
            ],
        },
        {
            id: 'open-inputs',
            title: 'Open Inputs and Set Values',
            paragraphs: ['Press Enter on a selected workflow to open inputs.', 'Review required fields and collect values from provider or manual entry paths.', 'Use manual entry when provider selection is not appropriate.'],
            callouts: [
                {type: 'expected', content: 'Required inputs are set and Run becomes available.'},
                {
                    type: 'recovery',
                    content: 'If input collection blocks a workflow run, fill missing required values and retry.'
                },
                {
                    type: 'screenshot',
                    label: 'Workflow Input View',
                    imageSrc: '/Oatty-workflow-collect-values-flow.webp',
                    imageAlt: 'Workflow input view screenshot',
                    content: 'Input list, and collector/manual entry paths.'
                },
                {
                    type: 'advanced',
                    content: 'Workflows can be defined so values are pulled in from a designated command and presented as a list of options to choose from.'
                },
            ],
        },
        {
            id: 'start-run',
            title: 'Start a Workflow Run',
            paragraphs: ['Run from the input view after required values are set.', 'Move to the run view and monitor step transitions.', 'Open step detail and logs for verification.'],
            callouts: [
                {type: 'expected', content: 'Run view shows workflow status and step-level execution progress.'},
                {
                    type: 'recovery',
                    content: 'If run fails early, inspect the first failing step detail and log message before rerunning.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-workflows-runner.png',
                    imageAlt: 'Workflow execution screenshot',
                    content: 'Capture active run view with step table, detail action, and log linkage.'
                },
                {
                    type: 'fallback',
                    content: 'CLI fallback: `oatty workflow list`, `oatty workflow preview <id>`, `oatty workflow run <id> --input key=value`.'
                },
            ],
        },
        {
            id: 'run-controls',
            title: 'Control Active Runs',
            paragraphs: ['Use Pause or Resume based on current run state.', 'Use Cancel when you need to stop execution.', 'Use Done to close completed runs.'],
            callouts: [
                {type: 'expected', content: 'Run control actions update run state and status messaging.'},
                {
                    type: 'recovery',
                    content: 'If a control is disabled, verify the current run state supports that action.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-workflows-runner.png',
                    imageAlt: 'Workflow execution screenshot',
                    content: 'Capture pause/resume/cancel controls across different run states.'
                },
                {type: 'advanced', content: 'Known limitation: step-level rerun/resume is not yet first-class.'},
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: [
                'Continue to Plugins to integrate plugin-backed tools used by workflows.',
                'Then continue to MCP HTTP Server to expose Oatty tools for MCP clients.',
                'Return to Search and Run Commands to validate command-level behavior used inside workflows.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can execute workflows repeatedly with predictable input and control behavior.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-workflows-runner.png',
                    imageAlt: 'Workflow execution screenshot',
                    content: 'Capture a completed run with terminal status and finalized step table.'
                },
            ],
        },
    ],
};
