import type {DocsPage} from '../types';

/**
 * Library and Catalogs page model.
 *
 * This page covers catalog lifecycle operations and base URL management in the TUI.
 */
export const libraryAndCatalogsPage: DocsPage = {
    path: '/docs/learn/library-and-catalogs',
    title: 'Library and Catalogs',
    summary: 'Manage catalogs as your command source of truth, including import, enablement, base URLs, and removal.',
    learnBullets: [
        'Import catalogs into the shared command registry.',
        'Toggle catalog enablement and verify active state.',
        'Define request headers such as Authorization in the headers editor.',
        'Set, add, and remove base URLs safely.',
        'Use CLI fallback for catalog import automation.',
    ],
    estimatedTime: '10-14 min',
    feedbackPrompt: 'Was this page helpful? Rate it or suggest improvements in docs feedback.',
    sections: [
        {
            id: 'overview',
            title: 'Overview',
            paragraphs: [
                'Catalogs are the source of truth for your command surface. Import OpenAPI schemas to create catalogs.',
                'Catalogs are shared across TUI, workflows, CLI, and MCP tooling.',
                'Catalogs can be enabled or disabled to control visibility in the TUI.',
                'Request headers such as Authorization and others can be set to support Vendor requirements.',
                'Base URLs can be specified to support multiple API endpoints for a catalog.',
                'Catalogs can be removed when no longer needed.',
            ],
            callouts: [
                {
                    type: 'expected',
                    content: 'Catalogs extend your command surface. No limits are placed on catalog count or size.'
                },
                {type: 'recovery', content: 'If catalogs are missing, verify the catalog import flow.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-complete-catalog-flow.webp',
                    imageAlt: 'Catalog import and configuration flow',
                    content: 'Catalog import and configuration flow.'
                },
            ],
        },
        {
            id: 'prerequisites',
            title: 'Prerequisites',
            paragraphs: ['Open the Library view.', 'Prepare an OpenAPI source path or URL for import.'],
            callouts: [
                {type: 'expected', content: 'Library opens with catalog list and catalog details panels available.'},
                {type: 'recovery', content: 'If Library is empty, import a catalog first.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture Library with list, details, and base URL areas visible.'
                },
            ],
        },
        {
            id: 'import-catalog',
            title: 'Import a Catalog',
            paragraphs: ['Select Import in Library.', 'Provide the schema source and complete the import flow.', 'Verify the new catalog appears in the list.'],
            callouts: [
                {type: 'expected', content: 'A new catalog entry appears and is selectable in Library.'},
                {type: 'recovery', content: 'If import fails, validate schema format and source path/URL, then retry.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-catalog-import.webp',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture successful import with the new catalog selected.'
                },
                {type: 'fallback', content: 'CLI fallback: `oatty import <path-or-url> --kind catalog`.'},
                {
                    type: 'advanced',
                    content: 'Imported catalogs become a shared command surface used by TUI, workflows, CLI, and MCP tooling.'
                },
            ],
        },
        {
            id: 'toggle-enablement',
            title: 'Toggle Catalog Enablement',
            paragraphs: ['Focus the catalog list and select a catalog.', 'Toggle enabled state from the list action path.', 'Confirm status changes in the catalog row and details.'],
            callouts: [
                {type: 'expected', content: 'Catalog status updates between enabled and disabled.'},
                {
                    type: 'recovery',
                    content: 'If status does not change, ensure the catalog row is focused before toggling.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture enabled and disabled states for the same catalog.'
                },
            ],
        },
        {
            id: 'headers-management',
            title: 'Define Request Headers',
            paragraphs: ['Open the catalog headers editor in Library.', 'Add header key-value entries, including `Authorization` when required.', 'Leave header values empty only when your API contract allows optional values.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'Header rows are visible in the catalog details and persist after focus changes.'
                },
                {
                    type: 'recovery',
                    content: 'If headers do not persist, check for invalid/empty header keys and correct the row.'
                },
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture headers editor with a valid Authorization header row.'
                },
                {type: 'advanced', content: 'Header validation enforces non-empty header keys before saving.'},
            ],
        },
        {
            id: 'base-url-management',
            title: 'Manage Base URLs',
            paragraphs: ['Select a catalog and open its base URL section.', 'Set the active base URL from the URL list.', 'Add or remove base URLs as needed.'],
            callouts: [
                {
                    type: 'expected',
                    content: 'One base URL is marked active and list updates reflect add/remove actions.'
                },
                {type: 'recovery', content: 'If updates fail validation, correct the base URL value and retry.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture base URL list with active selection and add/remove controls.'
                },
                {type: 'advanced', content: 'Base URL validation rejects invalid or empty URL sets for a catalog.'},
            ],
        },
        {
            id: 'remove-catalog',
            title: 'Remove a Catalog',
            paragraphs: ['Select the catalog to remove.', 'Trigger Remove and confirm in the modal.', 'Verify the catalog no longer appears in the list.'],
            callouts: [
                {type: 'expected', content: 'Selected catalog is removed from Library.'},
                {type: 'recovery', content: 'If remove is disabled, select a catalog row first.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture remove confirmation modal and post-remove list state.'
                },
                {
                    type: 'advanced',
                    content: 'Removal is destructive. Use confirmation flow to prevent accidental deletion.'
                },
            ],
        },
        {
            id: 'next-steps',
            title: 'Next Steps',
            paragraphs: ['Continue to Workflows Basics to run imported workflows with structured inputs.', 'Return to Search and Run Commands to validate command behavior against updated catalogs.'],
            callouts: [
                {type: 'expected', content: 'You can keep catalog state aligned with your execution workflows.'},
                {
                    type: 'screenshot',
                    imageSrc: '/Oatty-library.png',
                    imageAlt: 'Library catalog screenshot',
                    content: 'Capture final Library state with stable catalog and base URL configuration.'
                },
            ],
        },
    ],
};
