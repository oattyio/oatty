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

    private renderSectionHeading(section: DocsSection) {
        const headingLevel = this.sectionHeadingLevel(section);

        switch (headingLevel) {
            case 3:
                return html`<h3>${section.title}</h3>`;
            case 4:
                return html`<h4>${section.title}</h4>`;
            case 5:
                return html`<h5>${section.title}</h5>`;
            case 6:
                return html`<h6>${section.title}</h6>`;
            case 2:
            default:
                return html`<h2>${section.title}</h2>`;
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
                        <li>${bullet}</li>`)}
                </ul>
            </section>

            ${this.page.sections.map(
                    (section) => html`
                        <section id="${section.id}" class="m-docs-section">
                            ${this.renderSectionHeading(section)}
                            ${section.paragraphs.map((paragraph) => html`<p>${paragraph}</p>`)}
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
                                            <p>${callout.content}</p>
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
