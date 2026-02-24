import type {DocsPage} from '../types';

/**
 * Quick Start page model.
 *
 * This is the entry docs path and should remain action-oriented and short.
 */
export const quickStartPage: DocsPage = {
    path: '/docs/quick-start',
    title: 'Quick Start',
    summary: 'Start in the TUI, run a real command, run a workflow, then use CLI fallback for automation.',
    learnBullets: [
        'Use the TUI layout, focus movement, and help affordances with confidence.',
        'Import a catalog and discover commands through the interactive TUI path.',
        'Run a command and verify results in the logs and result views.',
        'Run a workflow end-to-end with structured inputs and step status.',
    ],
    estimatedTime: '10-15 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'install',
            title: 'Install Oatty',
            paragraphs: ['Install Oatty with npm for the fastest setup.', 'Run `oatty --help` to verify the install.'],
            codeSample: `npm install -g oatty\noatty --help`,
            callouts: [
                {type: 'expected', content: 'The `oatty --help` command prints usage and exits successfully.'},
                {
                    type: 'recovery',
                    content: 'If the command is missing, restart the shell and check your PATH. For source builds, run from the release binary path.',
                },
                {
                    type: 'fallback',
                    label: 'Alternative installation',
                    content: 'If npm is unavailable, build from source: `cargo build --release` and run `./target/release/oatty`.'
                },
            ],
        },
        {
            id: 'launch',
            title: 'Launch the TUI',
            paragraphs: ['Launch the interface with `oatty` in your terminal.', 'Identify the left navigation, main content pane, and the hints bar.'],
            codeSample: `oatty`,
            callouts: [
                {type: 'expected', content: 'The TUI opens with visible navigation and an empty Library view.'},
                {
                    type: 'screenshot',
                    label: 'Default TUI landing state',
                    imageSrc: '/assets/quick-start/oatty-first-launch.png',
                    imageAlt: 'Oatty TUI landing view with empty Library view',
                    content: 'Default TUI landing state with left nav and hints bar visible',
                },
                {
                    type: 'recovery',
                    content: 'If the UI does not render correctly, increase terminal size and relaunch. If colors are unreadable, verify your terminal supports 256 colors.'
                },
                {
                    type: 'advanced',
                    content: 'Power-user affordances: `Ctrl+L` toggles logs and `Ctrl+T` opens the theme picker when enabled.'
                },
            ],
        },
        {
            id: 'import_schema',
            title: 'Import Your First Catalog',
            paragraphs: [
                "Open the Library and look for the Import button. Tab until it's focused then press Enter, space bar or click with your mouse.",
            ],
            callouts: [
                {
                    type: 'screenshot',
                    label: 'Import file/URL picker',
                    imageSrc: '/assets/quick-start/oatty-import.png',
                    imageAlt: 'Oatty TUI import file picker with OpenAPI v3 schema selected from filesystem',
                    content: 'Import file picker with OpenAPI v3 schema selected from filesystem',
                },
                {
                    type: 'expected',
                    content: 'Oatty allows you to browse your filesystem or paste a URL and hit Enter or click the Open button to import.'
                },
                {
                    type: 'recovery',
                    content:
                        'If import fails, verify the schema path/URL and format. Oatty currently supports OpenAPI v3 in both yaml and json formats. Retry import from Library, or run the CLI fallback to inspect errors.',
                },
                {
                    type: 'fallback',
                    content: 'CLI import fallback: `oatty import <path-or-url> --kind catalog` (supports path and HTTP/HTTPS URL).'
                },
            ],
        },
        {
            id: 'optional_command_prefix',
            headingLevel: 3,
            title: 'Optional Command Prefix',
            paragraphs: [
                "An optional custom command prefix dialog will appear after choosing a file or pasting a URL. This allows you to customize the command prefix for the imported catalog, which can be useful for organizing commands or avoiding conflicts with existing commands.",
                "Skipping this step will derive the prefix from the schema directly."
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'An optional custom command prefix dialog will appear allowing you to customize the command prefix for the imported catalog.'
                },
                {
                    type: 'screenshot',
                    label: 'Custom command prefix dialog',
                    imageSrc: '/assets/quick-start/oatty-optional-prefix.png',
                    imageAlt: 'Optional custom command prefix dialog presented after importing an OpenAPI v3 schema',
                    content: 'Command prefix customization dialog',
                },
                {
                    type: 'expected',
                    content: 'The Library view updates with the imported catalog and shows the summary of what was imported.'
                },
                {
                    type: 'recovery',
                    content:
                        'If the custom prefix you enter is incorrect, you must remove the catalog and retry the import.',
                },
            ],
        },
        {
            id: 'complete_import',
            headingLevel: 3,
            title: 'Complete Import',
            paragraphs: [
                "The import process will complete after you press Enter or click the Open button. The Library view will update with the imported catalog and populate the details panel.",
            ],
            callouts: [
                {
                    type: 'screenshot',
                    label: 'Library view after import',
                    imageSrc: '/assets/quick-start/oatty-library-with-catalog.png',
                    imageAlt: 'Oatty TUI library view with the newly imported catalog',
                    content: 'Library view with the imported catalog and a populated details panel.',
                },
                {
                    type: 'recovery',
                    content:
                        'If the import fails, verify the schema path/URL and format. Oatty currently supports OpenAPI v3 in both yaml and json formats.',
                },
                {
                    type: 'advanced',
                    content: 'Advanced flow: add/remove catalogs and manage base URLs in Library; this is covered in Learn: Library and Catalogs.',
                    label: 'Configuration management',
                },
            ],
        },
        {
            id: 'run-command',
            title: 'Discover and Run a Command',
            paragraphs: ['Open Run Command and type a search phrase then press the Tab key to see matching result', 'Select a command, use Tab to see available flags and arg, input values and execute.', 'Inspect structured output and logs in the UI.'],
            callouts: [
                {type: 'expected', content: 'Command execution completes and results/logs show the final status.'},
                {
                    type: 'recovery',
                    content: 'If no command appears, confirm a catalog is imported. If execution fails, open command help and verify required inputs.'
                },
                {
                    type: 'screenshot',
                    label: 'Run Command view',
                    imageSrc: '/Oatty-run.png',
                    imageAlt: 'Run Command view showing execution output and logs',
                    content: 'Shows the command runner with completion list open and a second shot showing executed result/log output.',
                },
                {
                    type: 'fallback',
                    content: 'CLI fallback for the same action: run the selected command directly with required flags/args.'
                },
                {
                    type: 'recovery',
                    label: 'Command help',
                    content: 'Read the help (Ctrl+h) for the command to understand required inputs and flags. Verify the Auth header is configured and correct',
                },
                {
                    type: 'advanced',
                    content: 'For deeper discovery, use the Find/Browser view to inspect commands and send selected entries back to the runner.',
                    label: 'Advanced discovery',
                },
            ],
        },
    ],
};
