import {html, LitElement} from 'lit';

import {applyConstructibleStyles} from '../../styles/sheets/constructible-style';
import {docsViewStyle} from '../../styles/sheets/docs-view-sheet';
import {moduleStyle} from '../../styles/sheets/module-sheet';
import {themeStyle} from '../../styles/sheets/theme-sheet';
import {utilsStyle} from '../../styles/sheets/utils-sheet';
import {DocsCallout, DocsPage, DocsSection, DocsSectionHeadingLevel} from "../../docs/types";

/**
 * Reusable content renderer for docs pages.
 *
 * The outer docs shell (nav, TOC container, pagination) remains managed by the route host.
 */
export class DocsPageView extends LitElement {
    static properties = {
        page: {attribute: false},
    };

    declare page?: DocsPage;

    protected createRenderRoot(): ShadowRoot {
        return this.attachShadow({mode: 'open'});
    }

    connectedCallback(): void {
        super.connectedCallback();

        if (this.shadowRoot) {
            applyConstructibleStyles(this.shadowRoot, [themeStyle, utilsStyle, moduleStyle, docsViewStyle]);
        }
    }

    private openLightbox(callout: DocsCallout): void {
        if (!callout.imageSrc) {
            return;
        }

        this.dispatchEvent(
            new CustomEvent('docs-open-lightbox', {
                detail: {
                    src: callout.imageSrc,
                    alt: callout.imageAlt ?? this.calloutLabel(callout),
                },
                bubbles: true,
                composed: true,
            })
        );
    }

    private calloutLabel(callout: DocsCallout): string {
        if (callout.label?.trim()) {
            return callout.label;
        }

        switch (callout.type) {
            case 'expected':
                return 'Expected Result';
            case 'recovery':
                return 'If this fails';
            case 'screenshot':
                return 'Screenshot Target';
            case 'fallback':
                return 'CLI Fallback';
            case 'advanced':
                return 'Advanced';
            case 'tip':
                return 'Tip';
            default:
                return 'Note';
        }
    }

    private calloutIcon(callout: DocsCallout): string {
        switch (callout.type) {
            case 'expected':
                return 'check_circle';
            case 'recovery':
                return 'error';
            case 'screenshot':
                return 'image';
            case 'fallback':
                return 'terminal';
            case 'advanced':
                return 'psychology_alt';
            case 'tip':
                return 'tips_and_updates';
            default:
                return 'info';
        }
    }

    private calloutClass(callout: DocsCallout): string {
        const allowed = new Set(['expected', 'recovery', 'screenshot', 'fallback', 'advanced', 'tip']);
        const typeClass = allowed.has(callout.type) ? `m-docs-callout--${callout.type}` : 'm-docs-callout--generic';
        return `m-docs-callout ${typeClass}`;
    }

    private sectionHeadingLevel(section: DocsSection): DocsSectionHeadingLevel {
        return section.headingLevel ?? 2;
    }

    /**
     * Converts inline docs/runbook paths and HTTP(S) URLs into anchors.
     */
    private renderTextWithLinks(content: string) {
        const linkPattern = /`?(\/(?:docs|runbooks)\/[A-Za-z0-9._/#-]+|https?:\/\/[^\s`]+)`?/g;
        const renderedParts: unknown[] = [];
        let previousIndex = 0;

        for (const matchedLink of content.matchAll(linkPattern)) {
            const matchedText = matchedLink[0];
            const linkTarget = matchedLink[1];
            const matchIndex = matchedLink.index ?? 0;

            if (matchIndex > previousIndex) {
                renderedParts.push(content.slice(previousIndex, matchIndex));
            }

            const isExternalLink = linkTarget.startsWith('http://') || linkTarget.startsWith('https://');
            const linkLabel = this.humanReadableLinkLabel(linkTarget);
            renderedParts.push(
                isExternalLink
                    ? html`<a href="${linkTarget}" target="_blank" rel="noopener noreferrer">${linkLabel}</a>`
                    : html`<a href="${linkTarget}">${linkLabel}</a>`
            );

            previousIndex = matchIndex + matchedText.length;
        }

        if (previousIndex < content.length) {
            renderedParts.push(content.slice(previousIndex));
        }

        return renderedParts.length > 0 ? renderedParts : [content];
    }

    /**
     * Produces readable link text for docs/runbook/internal URL references.
     */
    private humanReadableLinkLabel(linkTarget: string): string {
        if (linkTarget.startsWith('http://') || linkTarget.startsWith('https://')) {
            try {
                const parsedUrl = new URL(linkTarget);
                return parsedUrl.hostname.replace(/^www\./, '');
            } catch {
                return linkTarget;
            }
        }

        const [pathPart, hashPart] = linkTarget.split('#');
        const pathSegments = pathPart.split('/').filter(Boolean);
        const lastPathSegment = pathSegments[pathSegments.length - 1] ?? pathPart;

        const normalizedSegment = lastPathSegment.replace(/\.md$/i, '').replaceAll('-', ' ');
        const formattedWords = normalizedSegment
            .split(/\s+/)
            .filter(Boolean)
            .map((word) => this.formatReadableWord(word));

        const baseLabel = formattedWords.join(' ');
        if (!hashPart) {
            return baseLabel;
        }

        const hashLabel = hashPart
            .replaceAll('-', ' ')
            .split(/\s+/)
            .filter(Boolean)
            .map((word) => this.formatReadableWord(word))
            .join(' ');

        return `${baseLabel} (${hashLabel})`;
    }

    private formatReadableWord(word: string): string {
        const uppercaseAcronyms = new Set(['api', 'cli', 'mcp', 'tui', 'sso', 'scim', 'yaml', 'json', 'slo']);
        if (uppercaseAcronyms.has(word.toLowerCase())) {
            return word.toUpperCase();
        }

        return word.charAt(0).toUpperCase() + word.slice(1).toLowerCase();
    }

    private sectionIcon(section: DocsSection): string | undefined {
        const title = section.title.toLowerCase();
        if (title.includes('prerequisite')) {
            return 'checklist';
        }
        if (title.includes('preflight')) {
            return 'fact_check';
        }
        if (title.includes('rollback')) {
            return 'undo';
        }
        if (title.includes('next steps')) {
            return 'arrow_forward';
        }
        if (title.includes('overview') || title.includes('purpose')) {
            return 'flag';
        }
        if (title.includes('step')) {
            return 'format_list_numbered';
        }
        if (title.includes('validate') || title.includes('verification') || title.includes('verify')) {
            return 'verified';
        }
        return undefined;
    }

    private renderSectionHeading(section: DocsSection) {
        const headingLevel = this.sectionHeadingLevel(section);
        const sectionIcon = this.sectionIcon(section);

        const headingContent = html`
            ${sectionIcon
                    ? html`
                        <span class="material-symbols-outlined m-docs-section__icon" aria-hidden="true">${sectionIcon}</span>
                    `
                    : ''}
                            <span>${section.title}</span>
        `;

        switch (headingLevel) {
            case 3:
                return html`<h3 class="m-docs-section__heading">${headingContent}</h3>`;
            case 4:
                return html`<h4 class="m-docs-section__heading">${headingContent}</h4>`;
            case 5:
                return html`<h5 class="m-docs-section__heading">${headingContent}</h5>`;
            case 6:
                return html`<h6 class="m-docs-section__heading">${headingContent}</h6>`;
            case 2:
            default:
                return html`<h2 class="m-docs-section__heading">${headingContent}</h2>`;
        }
    }

    /**
     * Smooth-scrolls to a section by id within the docs page content.
     *
     * Returns `true` when a target section is found and scrolled.
     */
    public scrollToSection(sectionId: string): boolean {
        const targetSection = this.shadowRoot?.getElementById(sectionId);
        if (!targetSection) {
            return false;
        }

        const preferReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
        targetSection.scrollIntoView({
            behavior: preferReducedMotion ? 'auto' : 'smooth',
            block: 'start',
        });

        return true;
    }

    protected render() {
        if (!this.page) {
            return html``;
        }

        return html`
            <header class="m-docs-header">
                <p class="m-docs-kicker">Docs</p>
                <h1 class="m-docs-title">${this.page.title}</h1>
                <p class="m-docs-summary">${this.page.summary}</p>
            </header>

            <section class="m-summary-card" aria-label="What you'll learn">
                <div class="m-summary-card__header">
                    <h2>What you'll learn</h2>
                    <span>${this.page.estimatedTime}</span>
                </div>
                <ul>
                    ${this.page.learnBullets.map((bullet) => html`
                        <li class="m-summary-card__bullet">
                            <span class="material-symbols-outlined m-summary-card__icon" aria-hidden="true">check_circle</span>
                            <span>${bullet}</span>
                        </li>`)}
                </ul>
            </section>

            ${this.page.sections.map(
                    (section) => html`
                        <section id="${section.id}" class="m-docs-section">
                            ${this.renderSectionHeading(section)}
                            ${section.paragraphs.map((paragraph) => html`<p>${this.renderTextWithLinks(paragraph)}</p>`)}
                            ${section.codeSample ? html`
                                <pre class="m-code"><code>${section.codeSample}</code></pre>` : ''}
                            ${(section.callouts ?? []).map(
                                    (callout) => html`
                                        <aside class="${this.calloutClass(callout)}"
                                               aria-label="${this.calloutLabel(callout)}">
                                            <h3 class="m-docs-callout__heading">
                                                <span class="material-symbols-outlined m-docs-callout__icon"
                                                      aria-hidden="true">${this.calloutIcon(callout)}</span>
                                                <span>${this.calloutLabel(callout)}</span>
                                            </h3>
                                            <p>${this.renderTextWithLinks(callout.content)}</p>
                                            ${callout.imageSrc
                                                    ? html`
                                                        <button
                                                                type="button"
                                                                class="m-docs-screenshot-trigger"
                                                                @click="${() => this.openLightbox(callout)}"
                                                                aria-label="Open screenshot in lightbox"
                                                        >
                                                            <img class="m-docs-screenshot-image"
                                                                 src="${callout.imageSrc}"
                                                                 alt="${callout.imageAlt ?? this.calloutLabel(callout)}"/>
                                                        </button>
                                                    `
                                                    : ''}
                                        </aside>
                                    `
                            )}
                        </section>
                    `
            )}

            ${this.page.feedbackPrompt
                    ? html`
                        <footer class="m-docs-feedback">
                            <p>${this.page.feedbackPrompt}</p>
                        </footer>
                    `
                    : ''}
        `;
    }
}

customElements.define('docs-page-view', DocsPageView);
