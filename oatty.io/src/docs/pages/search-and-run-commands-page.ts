import type { DocsPage } from '../types';

/**
 * Search and Run Commands page model.
 *
 * This page teaches the core command discovery and execution loop in the TUI.
 */
export const searchAndRunCommandsPage: DocsPage = {
  path: '/docs/learn/search-and-run-commands',
  title: 'Search and Run Commands',
  summary: 'Use the TUI command flow to find commands quickly, execute with confidence, and inspect results without leaving the interface.',
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
      id: 'prerequisites',
      title: 'Prerequisites',
      paragraphs: ['Launch Oatty with `oatty`.', 'Import at least one catalog so commands are discoverable.', 'Keep logs available for execution verification.'],
      codeSample: `oatty`,
      callouts: [
        { type: 'expected', content: 'You can open Run Command and see command suggestions from your imported catalog.' },
        { type: 'recovery', content: 'If no commands appear, import a catalog first in Library.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture Run Command focused with an empty input and visible hints.' },
        { type: 'fallback', content: 'Catalog import fallback: `oatty import <path-or-url> --kind catalog`.' },
      ],
    },
    {
      id: 'open-run-command',
      title: 'Step 1: Open Run Command',
      paragraphs: ['Navigate to Run Command from the left navigation.', 'Start typing your task phrase to query commands.'],
      callouts: [
        { type: 'expected', content: 'The command input is focused automatically and ready for text entry.' },
        { type: 'recovery', content: 'If typing does not update input, press Tab until the input area is focused.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture Run Command with focused input and active cursor.' },
      ],
    },
    {
      id: 'search-and-select',
      title: 'Step 2: Search and Select a Command',
      paragraphs: ['Type a task phrase such as `create app`.', 'Use Up and Down to change selection in the suggestion list.', 'Confirm the selected command before executing.'],
      callouts: [
        { type: 'expected', content: 'A relevant command is selected in the suggestion list.' },
        { type: 'recovery', content: 'If search returns nothing, verify catalog import and try broader search terms.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture suggestion list open with one highlighted command.' },
        { type: 'advanced', content: 'Selection behavior is focus-scoped. Hints remain the source of truth for active controls.' },
      ],
    },
    {
      id: 'review-help',
      title: 'Step 3: Review Help Before Running',
      paragraphs: ['Open command help from the active command context.', 'Verify required inputs and expected command shape.', 'Return to input and complete required values.'],
      callouts: [
        { type: 'expected', content: 'Required inputs are known before execution.' },
        { type: 'recovery', content: 'If help is unavailable, switch focus to the command area and read the hints bar for supported actions.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture command help visible with required input details.' },
        { type: 'advanced', content: 'Use this step to prevent avoidable execution failures from missing required arguments.' },
      ],
    },
    {
      id: 'execute-command',
      title: 'Step 4: Execute and Inspect Output',
      paragraphs: ['Run the selected command from the command runner.', 'Inspect structured output in the result view.', 'Open logs to verify completion or debug failures.'],
      callouts: [
        { type: 'expected', content: 'Execution reaches a terminal status, and output/logs show the final result.' },
        { type: 'recovery', content: 'If execution fails, read the first actionable log message, adjust required inputs, and rerun.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture executed result state and a selected log entry tied to the run.' },
        { type: 'fallback', content: 'Run the same command in CLI for scripts and CI with explicit flags and arguments.' },
      ],
    },
    {
      id: 'find-browser-handoff',
      title: 'Step 5: Use Find Browser for Discovery',
      paragraphs: ['Open Find to browse commands with summaries and categories.', 'Select a command and send it to Run Command.', 'Execute from Run Command after reviewing inputs.'],
      callouts: [
        { type: 'expected', content: 'A command selected in Find appears in Run Command ready for execution.' },
        { type: 'recovery', content: 'If handoff does not occur, confirm focus is in Find and retry the handoff action shown in hints.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture Find browser with selected command and the post-handoff Run Command state.' },
        { type: 'advanced', content: 'Find is best for exploration; Run Command is optimized for fast execution loops.' },
      ],
    },
    {
      id: 'cli-fallback',
      title: 'CLI Fallback for Automation',
      paragraphs: ['Use CLI search when you need non-interactive discovery.', 'Run commands directly in scripts and CI with explicit inputs.'],
      codeSample: `oatty search "create app"\noatty apps create --name demo-app`,
      callouts: [
        { type: 'expected', content: 'You can execute the same command path outside the TUI.' },
        { type: 'recovery', content: 'If CLI execution differs from TUI expectation, verify command arguments and active catalog configuration.' },
        { type: 'advanced', content: 'Use TUI for discovery and validation first, then promote stable command lines into automation.' },
      ],
    },
    {
      id: 'next-steps',
      title: 'Next Steps',
      paragraphs: ['Continue to Library and Catalogs to manage command sources.', 'Then continue to Workflows to compose repeatable multi-step execution.'],
      callouts: [
        { type: 'expected', content: 'You can discover and run commands reliably in both TUI and CLI contexts.' },
        { type: 'screenshot', imageSrc: '/Oatty-run.png', imageAlt: 'Run command screenshot', content: 'Capture a completed run state that includes selected command, output, and logs.' },
      ],
    },
  ],
};
