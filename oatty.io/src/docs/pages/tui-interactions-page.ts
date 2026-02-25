import type {DocsPage} from '../types';

/**
 * TUI interactions reference page model.
 *
 * This page is a lookup reference for keyboard navigation, focus behavior, and
 * high-frequency interaction patterns in Oatty TUI mode.
 */
export const tuiInteractionsPage: DocsPage = {
    path: '/docs/reference/tui-interactions',
    title: 'TUI Interactions Reference',
    summary: 'Use this page as a fast reference for keybindings, focus movement, and view-level interaction rules in Oatty.',
    learnBullets: [
        'Navigate major views with predictable focus behavior.',
        'Use command and logs shortcuts without leaving the keyboard.',
        'Interpret hint bars and context-sensitive actions.',
        'Recover quickly when focus or state appears inconsistent.',
    ],
    estimatedTime: '7-10 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'core-navigation',
            title: 'Core Navigation Model',
            paragraphs: [
                'The TUI follows a consistent focus ring model across panels.',
                'Use Tab and BackTab to move focus through interactive areas.',
                'Use Enter to activate focused controls and Esc to dismiss overlays or modals.',
            ],
            codeSample: `Tab      -> Next focus target
Shift+Tab -> Previous focus target
Enter    -> Activate focused control
Esc      -> Close modal / return`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Focus outline and hint text should always reflect the active interaction target.'
                },
                {
                    type: 'recovery',
                    content: 'If focus appears lost, press Esc once or twice, then Tab to re-enter the expected focus cycle.'
                },
            ],
        },
        {
            id: 'global-shortcuts',
            title: 'Global Shortcuts',
            paragraphs: [
                'Global shortcuts provide fast access to high-frequency panels.',
                'These shortcuts are intended to reduce context switching during execution and debugging.',
            ],
            codeSample: `Ctrl+L -> Toggle logs panel
Ctrl+T -> Open theme picker
F1     -> Open command help (contextual)
`,
            callouts: [
                {
                    type: 'tip',
                    content: 'Use logs and help shortcuts before rerunning failing commands to reduce trial-and-error loops.'
                },
            ],
        },
        {
            id: 'run-command-flow',
            title: 'Run Command Interaction Flow',
            paragraphs: [
                'Type search text in Run Command to open suggestions.',
                'Use arrow keys to change selection and Enter to confirm.',
                'Complete required inputs, run the command, and inspect results and logs.',
            ],
            codeSample: `# Typical loop
1) Type intent text
2) Tab / arrows through suggestions
3) Enter to select
4) Fill required args/flags
5) Execute and inspect output`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Selected command details and execution output remain visible in the same workflow.'
                },
                {
                    type: 'advanced',
                    content: 'Use Find/Browser for broader discovery, then hand selected commands back into Run Command.'
                },
            ],
        },
        {
            id: 'workflows-and-collector',
            title: 'Workflows and Input Collector',
            paragraphs: [
                'Workflow runs move through list selection, input collection, and execution monitoring.',
                'Collector screens enforce required inputs before enabling run.',
                'When provider-backed choices exist, select from list options before falling back to manual entry.',
            ],
            codeSample: `# Typical workflow loop
1) Select workflow
2) Open inputs
3) Provide required values
4) Run
5) Monitor step status and logs`,
            callouts: [
                {
                    type: 'expected',
                    content: 'Run actions become available only after required input validation passes.'
                },
                {
                    type: 'recovery',
                    content: 'If run remains disabled, inspect collector errors and resolve missing required values first.'
                },
            ],
        },
        {
            id: 'mouse-and-accessibility',
            title: 'Mouse and Accessibility Behavior',
            paragraphs: [
                'Oatty supports mouse interactions for selection and button activation where available.',
                'Keyboard access remains first-class and should always provide a complete interaction path.',
                'Use large enough terminal dimensions to avoid clipped content and hidden controls.',
            ],
            callouts: [
                {
                    type: 'tip',
                    content: 'If terminal rendering looks inconsistent, resize first, then reopen the current view before troubleshooting deeper issues.'
                },
            ],
        },
    ],
};
