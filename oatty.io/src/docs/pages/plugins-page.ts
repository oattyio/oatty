import type { DocsPage } from '../types';

/**
 * Plugins page model.
 *
 * This page covers plugin table, details, and editor operations in the TUI.
 */
export const pluginsPage: DocsPage = {
  path: '/docs/learn/plugins',
  title: 'Plugins',
  summary: 'Manage plugin lifecycle and configuration from one TUI workflow, including details, validation, and save paths.',
  learnBullets: [
    'Inspect plugin inventory and open plugin details.',
    'Start, stop, and restart plugins from table and details contexts.',
    'Validate and save plugin editor updates safely.',
    'Define remote headers or local env vars in editor key-value rows.',
  ],
  estimatedTime: '10-14 min',
  feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
  sections: [
    {
      id: 'prerequisites',
      title: 'Prerequisites',
      paragraphs: ['Open the Plugins view.', 'Confirm at least one plugin entry is present.'],
      callouts: [
        { type: 'expected', content: 'Plugin table loads with search, list, and action controls.' },
        { type: 'recovery', content: 'If no plugins are listed, create or import plugin definitions first.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture plugin table with selected row and visible actions.' },
      ],
    },
    {
      id: 'plugin-table-operations',
      title: 'Step 1: Use Plugin Table Operations',
      paragraphs: ['Select a plugin row from the table.', 'Open details from the selected plugin.', 'Use start, stop, and restart actions from the table-level controls.'],
      callouts: [
        { type: 'expected', content: 'Selected plugin actions execute and status updates reflect control operations.' },
        { type: 'recovery', content: 'If actions are unavailable, verify a plugin row is selected and supports the target action.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture selected plugin row, details open action, and start/stop controls.' },
        { type: 'advanced', content: 'Details, edit, and start/stop/restart are available through dedicated hotkeys in focused table context.' },
      ],
    },
    {
      id: 'plugin-details',
      title: 'Step 2: Inspect Plugin Details',
      paragraphs: ['Open plugin details from the selected row.', 'Review metadata, logs, and exposed tool information.', 'Run control operations from details when needed.'],
      callouts: [
        { type: 'expected', content: 'Details modal loads plugin metadata and tool/log sections for the selected plugin.' },
        { type: 'recovery', content: 'If details fail to load, refresh details and verify plugin is still selected.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture plugin details modal with loaded data and control hints visible.' },
        { type: 'advanced', content: 'Details includes explicit error rendering when detail loading fails.' },
      ],
    },
    {
      id: 'plugin-editor',
      title: 'Step 3: Use Plugin Editor Validate and Save',
      paragraphs: ['Open plugin editor for add/edit flows.', 'Validate configuration before save.', 'Save only after required fields and validation pass.'],
      callouts: [
        { type: 'expected', content: 'Validation feedback is shown, and save persists valid plugin configuration.' },
        { type: 'recovery', content: 'If save is disabled, resolve validation issues and required fields first.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture editor form, validation state, and enabled save button.' },
        { type: 'advanced', content: 'Validate and Save availability is focus-aware and state-dependent.' },
      ],
    },
    {
      id: 'plugin-config-headers-env',
      title: 'Step 4: Define Headers or Env Vars',
      paragraphs: [
        'Use Remote transport to define request headers in the key-value editor.',
        'Use Local transport to define environment variables in the same editor.',
        'Add required auth values such as `Authorization` for remote integrations when needed.',
      ],
      callouts: [
        { type: 'expected', content: 'Key-value rows persist and match the selected transport mode.' },
        { type: 'recovery', content: 'If configuration fails validation, correct invalid or empty keys and validate again.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture plugin editor showing Remote headers with an Authorization row.' },
        { type: 'advanced', content: 'The key-value editor label switches by transport: Headers for Remote, Env Vars for Local.' },
      ],
    },
    {
      id: 'next-steps',
      title: 'Next Steps',
      paragraphs: ['Continue to MCP HTTP Server to expose Oatty tools over a local MCP endpoint.', 'Return to Workflows Basics to combine plugin-backed tools with workflow execution.'],
      callouts: [
        { type: 'expected', content: 'You can operate plugin lifecycle and configuration with predictable outcomes.' },
        { type: 'screenshot', imageSrc: '/docs-screenshot-placeholder.svg', imageAlt: 'Screenshot placeholder', content: 'Capture final plugin operational state with clear status indicators.' },
      ],
    },
  ],
};
