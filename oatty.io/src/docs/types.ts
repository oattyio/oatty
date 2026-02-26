/**
 * Structural content model for docs pages rendered inside the docs shell.
 */
export type DocsCalloutType = 'expected' | 'recovery' | 'fallback' | 'advanced' | 'tip' | string;

/**
 * Flexible callout metadata rendered below section content.
 */
export type DocsCallout = {
    type: DocsCalloutType;
    content: string;
    label?: string;
} & ({
    imageSrc?: never;
    imageAlt?: never;
} | {
    type: 'screenshot',
    imageSrc: string;
    imageAlt: string;
});

/**
 * Allowed heading levels for section titles within a docs page.
 *
 * `h1` is reserved for the page title.
 */
export type DocsSectionHeadingLevel = 2 | 3 | 4 | 5 | 6;

/**
 * Structural content model for docs sections rendered inside a docs page.
 */
export type DocsSection = {
    id: string;
    title: string;
    tocTitle?: string;
    headingLevel?: DocsSectionHeadingLevel;
    paragraphs: string[];
    codeSample?: string;
    callouts?: DocsCallout[];
};

/**
 * Content contract for a docs page in the `/docs/...` route space.
 */
export type DocsPage = {
    path: string;
    title: string;
    summary: string;
    learnBullets: string[];
    estimatedTime: string;
    sections: DocsSection[];
    feedbackPrompt?: string;
};
