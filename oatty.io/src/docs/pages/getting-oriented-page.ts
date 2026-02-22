import type {DocsPage} from '../types';

/**
 * Getting Oriented page model.
 *
 * This page establishes baseline TUI fundamentals before deeper feature modules.
 */
export const gettingOrientedPage: DocsPage = {
    path: '/docs/learn/getting-oriented',
    title: 'Getting Oriented',
    summary: 'Learn the core interaction model so you can navigate Oatty quickly and recover from common UI friction.',
    learnBullets: [
        'Move focus predictably with keyboard and mouse.',
        'Use logs, hints, and help affordances during execution.',
        'Keep a stable mental model across views and modals.',
    ],
    estimatedTime: '8-12 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            paragraphs: ['Launch the TUI with `oatty`.', 'Use a terminal size that shows navigation, content, and hints clearly.', 'Confirm your keyboard sends Tab, Shift+Tab, and Esc correctly.'],
            codeSample: `oatty`,
            callouts: [
                {type: 'expected', content: 'The default TUI view opens with visible focus and hints.'},
                {
                    type: 'recovery',
                    content: 'If rendering clips, resize the terminal and relaunch. If key input is inconsistent, verify terminal key settings.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-first-run.png',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'The default landing view with focus outline visible.'
                },
            ],
        },
        {
            id: 'navigation-model',
            title: 'Navigation Model',
            paragraphs: ['Use the left navigation to switch top-level views.', 'Treat each view as a focused workspace with shared interaction rules.', 'Return to the same view repeatedly to build speed.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can move between Library, Run Command, Find, Workflows, Plugins, and MCP Server without confusion.'
                },
                {
                    type: 'recovery',
                    content: 'If a view does not react to input, cycle focus with Tab until the target region highlights.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-nav.webp',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'Capture left navigation with one selected view and one hovered view.'
                },
                {type: 'advanced', content: 'Most views use Tab and Shift+Tab as the primary focus-cycle pattern.'},
            ],
        },
        {
            id: 'keyboard-focus',
            title: 'Keyboard and Focus',
            paragraphs: [
                'Press Tab to move focus forward.',
                'Press Shift+Tab to move focus backward.',
                'When a list is focused, press Up and Down to move one row.',
                'When a long list is focused, press PgUp and PgDown to move faster.',
                'List navigation keys are focus-scoped, and the hints bar remains the source of truth for active view behavior.',
                'Press Esc to close modals and transient overlays.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Focusable areas highlight consistently, and modal dismissal works with Esc.'
                },
                {
                    type: 'recovery',
                    content: 'If focus appears stuck, close overlays with Esc, then cycle Tab until the intended element gains focus.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-keyboard-focus.webp',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'States showing focus moving across different interactive regions.'
                },
                {
                    type: 'advanced',
                    content: 'Hint spans show context-sensitive actions; base focus movement is intentionally omitted from hints in many views.'
                },
            ],
        },
        {
            id: 'mouse-interaction',
            title: 'Mouse Interaction',
            paragraphs: ['Click list rows to select entries.', 'Click buttons to trigger the same action exposed through keyboard controls.', 'Use mouse selection for quick scanning and keyboard for repetitive execution.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Clicking an interactive element updates focus and action state predictably.'
                },
                {
                    type: 'recovery',
                    content: 'If clicks do not act on the expected element, click once to focus the panel, then click the target action again.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-mouse.webp',
                    imageAlt: 'Oatty UI screencapture showing mouse interactions',
                    content: 'List selection and hover state in the command finder view.'
                },
                {
                    type: 'advanced',
                    content: 'Some modal flows intentionally close through mouse clicks; Esc remains the global close behavior.'
                },
            ],
        },
        {
            id: 'logs-panel',
            title: 'Logs and Inspection',
            paragraphs: ['Toggle logs with Ctrl+L.', 'Use logs to verify command/workflow status and inspect failures.', 'Filter and inspect entries before rerunning actions.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'The logs panel opens and closes without losing your current workflow context.'
                },
                {
                    type: 'recovery',
                    content: 'If no entries appear, execute a command first. If the panel feels unresponsive, refocus it with Tab before filtering.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-logs.webp',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'Capture logs closed and logs open with one selected log entry.'
                },
                {
                    type: 'fallback',
                    content: 'For non-interactive automation logs, run commands with CLI output and collect logs in your shell/CI system.'
                },
                {
                    type: 'advanced',
                    content: 'Layout can place logs differently at wider terminal sizes while preserving the same interaction model.'
                },
            ],
        },
        {
            id: 'help',
            title: 'Hints and Help',
            paragraphs: ['Read the hints bar before executing an unfamiliar action.', 'Use in-view help to confirm expected key and mouse behavior.', 'Treat hints as the fastest way to know which actions are available in the active view.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can identify available actions in the active view without leaving the screen.'
                },
                {
                    type: 'recovery',
                    content: 'If hints do not match behavior, confirm the active focus area. Hints are context-sensitive.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-keyboard-focus.webp',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'Capture the hints bar while focus is on a list, then on an action button.'
                },
                {
                    type: 'advanced',
                    content: 'Parent components may own shared hotkeys in specific flows; this is an intentional exception pattern.'
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: ['Continue to Search and Run Commands for deeper execution flow.', 'Then move to Library and Workflows to build repeatable operations.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can navigate the TUI confidently and continue through feature modules faster.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-finder.png',
                    imageAlt: 'Oatty UI screenshot',
                    content: 'Capture the final oriented state with a selected view and visible hints.'
                },
            ],
        },
    ],
};
