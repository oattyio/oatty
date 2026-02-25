import type {DocsPage} from '../types';

/**
 * Plugins page model.
 *
 * This page covers plugin table, details, and editor operations in the TUI.
 */
export const pluginsPage: DocsPage = {
    path: '/docs/learn/plugins',
    title: 'MCP Plugins',
    summary: 'Manage plugin lifecycle and configuration from one TUI workflow, including details, validation, and save paths.',
    learnBullets: [
        'Extend Oatty\'s capabilities with your favorite MCP servers.',
        'Inspect MCP inventory and open MCP details.',
        'Start, stop, and restart MCP from table and details contexts.',
        'Validate and save editor updates safely.',
        'Define remote headers or local env vars in editor key-value rows.',
    ],
    estimatedTime: '10-14 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'overview',
            title: 'Overview',
            paragraphs: [
                'Plugins are a powerful way to extend Oatty\'s capabilities with your favorite MCP servers (http or stdio).',
                'Use the Plugins view to manage plugin lifecycle and configuration from one TUI workflow.',
                'Details and validation errors are surfaced in the TUI, and credentials are stored using your OS keychain where available.',
                'Define remote headers or local env vars in the key-value editor to support vendor requirements.',
            ],
            callouts: [
                {
                    type: 'expected',
                    label: 'Goal',
                    content: 'Use MCP servers to extend Oatty\'s capabilities.'
                },
                {
                    type: 'screenshot',
                    label: 'MCP Plugin Flow',
                    imageSrc: '/Oatty-mcp-flow.webp',
                    imageAlt: 'MCP plugin flow',
                    content: 'Complete MCP plugin flow that includes plugin table, details, and editor.'
                },
            ]
        },
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            paragraphs: ['Open the Plugins view.', 'Confirm at least one plugin entry is present.'],
            callouts: [
                {type: 'expected', content: 'Plugin table loads with search, list, and action controls.'},
                {type: 'recovery', content: 'If no plugins are listed, create or import plugin definitions first.'},
                {
                    type: 'screenshot',
                    label: 'MCP Plugin Table',
                    imageSrc: '/Oatty-mcp-servers.png',
                    imageAlt: 'Oatty MCP plugin table screenshot',
                    content: 'Plugin table with search, list, and action controls.'
                },
            ],
        },
        {
            id: 'add-plugin-editor',
            title: 'Add or Edit a Plugin Configuration',
            paragraphs: [
                'Open plugin editor for add/edit flows.',
                'Use Remote transport to define request headers in the key-value editor.',
                'Use Local transport to define environment variables in the same editor.',
                'Add required auth values such as `Authorization` for remote integrations when needed.',
                'Validate configuration before save.',
                'Save after required fields and validation pass.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Validation feedback is shown, and save persists valid plugin configuration.'
                },
                {
                    type: 'recovery',
                    content: 'If save is disabled, resolve validation issues and required fields first.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-add-mcp-server-flow.webp',
                    imageAlt: 'Oatty add MCP server flow',
                    content: 'Editor form, validation state, and enabled save button.'
                },
                {
                    type: 'advanced',
                    content: 'The key-value editor label switches by transport: Headers for Remote, Env Vars for Local.'
                },
            ],
        },
        {
            id: 'plugin-table-operations',
            title: 'Use Plugin Table Operations',
            paragraphs: ['Select a plugin row from the table.', 'Open details from the selected plugin.', 'Use start, stop, and restart actions from the table-level controls.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Selected plugin actions execute and status updates reflect control operations.'
                },
                {
                    type: 'recovery',
                    content: 'If actions are unavailable, verify a plugin row is selected and supports the target action.'
                },
                {
                    type: 'screenshot',
                    label: 'MCP Plugin Table',
                    imageSrc: '/Oatty-mcp-table-actions.webp',
                    imageAlt: 'Oatty MCP plugin table view',
                    content: 'Selected plugin row, details open action, and start/stop controls.'
                },
                {
                    type: 'advanced',
                    content: 'Details, edit, and start/stop/restart are available through dedicated hotkeys in focused table context.'
                },
            ],
        },
        {
            id: 'plugin-details',
            title: 'Inspect Plugin Details',
            paragraphs: ['Open plugin details from the selected row.', 'Review metadata, logs, and exposed tool information.', 'Run control operations from details when needed.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Details modal loads plugin metadata and tool/log sections for the selected plugin.'
                },
                {
                    type: 'recovery',
                    content: 'If details fail to load, refresh details and verify plugin is still selected.'
                },
                {
                    type: 'screenshot',
                    label: 'MCP Plugin Details',
                    imageSrc: '/Oatty-mcp-details-view.png',
                    imageAlt: 'Oatty MCP plugin details view screenshot',
                    content: 'Plugins details modal with metadata, tool, and log sections.'
                },
                {type: 'advanced', content: 'Details includes explicit error rendering when detail loading fails.'},
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: ['Continue to MCP HTTP Server to expose Oatty tools over a local MCP endpoint.', 'Return to Workflows Basics to combine plugin-backed tools with workflow execution.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'You can operate plugin lifecycle and configuration with predictable outcomes.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-plugins-table-view.png',
                    imageAlt: 'Screenshot placeholder',
                    content: 'Capture final plugin operational state with clear status indicators.'
                },
            ],
        },
    ],
};
