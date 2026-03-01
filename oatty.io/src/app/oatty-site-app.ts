import {html, LitElement} from 'lit';

import '../components/docs/docs-page-view';
import {docsPages} from '../docs/pages';
import type {DocsPage} from '../docs/types';
import {applyConstructibleStyles} from '../styles/sheets/constructible-style';
import {baseStyle} from '../styles/sheets/base-sheet';
import {docsViewStyle} from '../styles/sheets/docs-view-sheet';
import {layoutStyle} from '../styles/sheets/layout-sheet';
import {moduleStyle} from '../styles/sheets/module-sheet';
import {stateStyle} from '../styles/sheets/state-sheet';
import {themeStyle} from '../styles/sheets/theme-sheet';
import {utilsStyle} from '../styles/sheets/utils-sheet';

type DocsPageViewElement = HTMLElement & {
    scrollToSection?: (sectionId: string) => boolean;
};

const DEFAULT_PAGE_TITLE = 'Oatty - Unified TUI & CLI for OpenAPI APIs | Schema-Driven Discovery & Workflows';
const DEFAULT_PAGE_DESCRIPTION = 'Schema-driven OpenAPI command discovery, workflow orchestration, and MCP execution with reviewable safety gates.';
const SITE_ORIGIN = 'https://oatty.io';

const DOCS_NAV = [
    {
        section: 'Get Started',
        links: [{title: 'Quick Start', path: '/docs/quick-start'}],
    },
    {
        section: 'Guides',
        links: [
            {title: 'Bootstrap Sentry with an Agent + MCP', path: '/docs/guides/sentry-bootstrap'},
            {title: 'Sentry + Datadog + PagerDuty Playbook', path: '/docs/guides/sentry-datadog-pagerduty-playbook'},
            {title: 'Vercel -> Render Migration Playbook', path: '/docs/guides/vercel-to-render-migration-playbook'},
            {title: 'Access Review Collection Playbook', path: '/docs/guides/access-review-collection-playbook'},
            {
                title: 'Credential Rotation Readiness Playbook',
                path: '/docs/guides/credential-rotation-readiness-playbook'
            },
        ],
    },
    {
        section: 'Learn',
        links: [
            {title: 'How Oatty Executes Safely', path: '/docs/learn/how-oatty-executes-safely'},
            {title: 'Getting Oriented', path: '/docs/learn/getting-oriented'},
            {title: 'Library and Catalogs', path: '/docs/learn/library-and-catalogs'},
            {title: 'Search and Run Commands', path: '/docs/learn/search-and-run-commands'},
            {title: 'Workflows Basics', path: '/docs/learn/workflows-basics'},
            {title: 'Plugins', path: '/docs/learn/plugins'},
            {title: 'MCP HTTP Server', path: '/docs/learn/mcp-http-server'},
        ],
    },
    {
        section: 'Reference',
        links: [
            {title: 'CLI Commands', path: '/docs/reference/cli-commands'},
            {title: 'TUI Interactions', path: '/docs/reference/tui-interactions'},
        ],
    },
];

export class OattySiteApp extends LitElement {
    private currentPath = this.normalizePath(window.location.pathname);
    private isHeroInstallExpanded = false;

    private readonly onPopState = () => {
        this.currentPath = this.normalizePath(window.location.pathname);
        this.updateSearchMetadataForRoute();
        this.requestUpdate();
    };

    protected createRenderRoot(): ShadowRoot {
        return this.attachShadow({mode: 'open'});
    }

    connectedCallback(): void {
        super.connectedCallback();

        if (this.shadowRoot) {
            applyConstructibleStyles(this.shadowRoot, [baseStyle, themeStyle, utilsStyle, layoutStyle, moduleStyle, stateStyle, docsViewStyle]);
        }

        window.addEventListener('popstate', this.onPopState);
        this.updateSearchMetadataForRoute();
    }

    disconnectedCallback(): void {
        window.removeEventListener('popstate', this.onPopState);
        super.disconnectedCallback();
    }

    private openLightbox(src: string, alt: string) {
        const lightbox = this.shadowRoot?.querySelector('.m-lightbox');
        const img = lightbox?.querySelector('img');
        if (lightbox && img) {
            img.setAttribute('src', src);
            img.setAttribute('alt', alt);
            lightbox.classList.add('is-open');
        }
    }

    private closeLightbox() {
        const lightbox = this.shadowRoot?.querySelector('.m-lightbox');
        if (lightbox) {
            lightbox.classList.remove('is-open');
        }
    }

    private handleDocsOpenLightbox(event: CustomEvent<{ src: string; alt: string }>): void {
        if (!event.detail?.src) {
            return;
        }

        this.openLightbox(event.detail.src, event.detail.alt ?? 'Screenshot');
    }

    private renderLightbox() {
        return html`

            <div class="m-lightbox" @click="${this.closeLightbox}">
                <div class="m-lightbox__close" aria-label="Close lightbox">✕</div>
                <img src="" alt="" @click="${(event: Event) => event.stopPropagation()}"/>
            </div>
        `;
    }

    private smoothScrollToSection(event: Event, sectionId: string): void {
        event.preventDefault();
        const section = this.shadowRoot?.getElementById(sectionId);
        if (!section) {
            return;
        }

        section.scrollIntoView({behavior: 'smooth', block: 'start'});
        history.replaceState(null, '', `#${sectionId}`);
    }

    private toggleHeroInstallPanel(event: Event): void {
        event.preventDefault();
        this.isHeroInstallExpanded = !this.isHeroInstallExpanded;
        this.requestUpdate();
    }

    private normalizePath(pathname: string): string {
        if (!pathname || pathname === '/') {
            return '/';
        }

        const trimmed = pathname.replace(/\/+$/, '');
        if (trimmed === '/docs') {
            return '/docs/quick-start';
        }

        return trimmed;
    }

    private isDocsRoute(): boolean {
        return this.currentPath.startsWith('/docs');
    }

    private navigate = (event: Event) => {
        const anchor = event.currentTarget as HTMLAnchorElement | null;
        const href = anchor?.getAttribute('href');
        if (!href || href.startsWith('http') || href.startsWith('#')) {
            return;
        }

        event.preventDefault();
        history.pushState({}, '', href);
        this.currentPath = this.normalizePath(window.location.pathname);
        this.updateSearchMetadataForRoute();
        window.scrollTo({top: 0});
        this.requestUpdate();
    };

    private updateSearchMetadataForRoute(): void {
        const routePath = this.currentPath === '/docs' ? '/docs/quick-start' : this.currentPath;
        const canonicalUrl = `${SITE_ORIGIN}${routePath}`;

        if (this.isDocsRoute()) {
            const page = this.currentDocsPage();
            const docsTitle = page ? `${page.title} | Oatty Docs` : 'Oatty Docs | One CLI for Every API';
            const docsDescription = page?.summary ?? 'Documentation for Oatty, the schema-driven CLI, TUI, and MCP command surface.';
            this.applyDocumentMetadata(docsTitle, docsDescription, canonicalUrl);
            return;
        }

        this.applyDocumentMetadata(DEFAULT_PAGE_TITLE, DEFAULT_PAGE_DESCRIPTION, canonicalUrl);
    }

    private applyDocumentMetadata(title: string, description: string, canonicalUrl: string): void {
        document.title = title;
        this.updateMetaContent('name', 'title', title);
        this.updateMetaContent('name', 'description', description);
        this.updateMetaContent('property', 'og:title', title);
        this.updateMetaContent('property', 'og:description', description);
        this.updateMetaContent('property', 'og:url', canonicalUrl);
        this.updateMetaContent('name', 'twitter:title', title);
        this.updateMetaContent('name', 'twitter:description', description);
        this.updateMetaContent('name', 'twitter:url', canonicalUrl);

        const canonicalLinkElement = document.querySelector('link[rel="canonical"]');
        if (canonicalLinkElement instanceof HTMLLinkElement) {
            canonicalLinkElement.href = canonicalUrl;
        }
    }

    private updateMetaContent(attributeName: 'name' | 'property', selectorValue: string, content: string): void {
        const selector = `meta[${attributeName}="${selectorValue}"]`;
        const metaElement = document.querySelector(selector);
        if (metaElement instanceof HTMLMetaElement) {
            metaElement.content = content;
        }
    }

    private handleTableOfContentsClick(event: Event, sectionId: string): void {
        event.preventDefault();

        const docsPageView = this.shadowRoot?.querySelector('docs-page-view') as DocsPageViewElement | null;
        const sectionWasScrolled = docsPageView?.scrollToSection?.(sectionId) ?? false;
        if (!sectionWasScrolled) {
            return;
        }

        history.replaceState({}, '', `${this.currentPath}#${sectionId}`);
    }

    private currentDocsPage(): DocsPage | undefined {
        return docsPages.find((page) => page.path === this.currentPath);
    }

    private docsNeighborPages() {
        const index = docsPages.findIndex((page) => page.path === this.currentPath);
        if (index < 0) {
            return {previous: undefined, next: undefined};
        }
        return {
            previous: docsPages[index - 1],
            next: docsPages[index + 1],
        };
    }

    private renderDocs() {
        const page = this.currentDocsPage();
        if (!page) {
            return html`
                <a class="m-skip-link" href="#main-content">Skip to content</a>
                <header class="l-header">
                    <div class="l-shell l-header__inner">
                        <a href="/" @click="${this.navigate}" class="m-logo" aria-label="Oatty home">
                            <img src="/icons/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;"/>
                            <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
                        </a>
                        <div class="m-header-actions">
                            <a class="m-button" href="/" @click="${this.navigate}">Back to Home</a>
                        </div>
                    </div>
                </header>
                <main id="main-content" class="l-main">
                    <div class="l-shell">
                        <article class="m-card">
                            <a class="m-button m-button--primary" href="/docs/quick-start" @click="${this.navigate}">Go
                                to Quick Start</a>
                        </article>
                    </div>
                </main>
            `;
        }

        const {previous, next} = this.docsNeighborPages();

        return html`
            <a class="m-skip-link" href="#main-content">Skip to content</a>
            <header class="l-header">
                <div class="l-shell l-header__inner">
                    <a href="/" @click="${this.navigate}" class="m-logo" aria-label="Oatty home">
                        <img src="/icons/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;"/>
                        <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
                    </a>
                    <nav class="m-nav" aria-label="Primary">
                        <a class="m-nav__link" href="/docs/quick-start" @click="${this.navigate}">Quick Start</a>
                        <a class="m-nav__link" href="/" @click="${this.navigate}">Home</a>
                        <a class="m-nav__link" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">GitHub</a>
                    </nav>
                </div>
            </header>

            <main id="main-content" class="l-main">
                <div class="l-shell l-docs-layout">
                    <aside class="m-docs-sidebar" aria-label="Docs navigation">
                        ${DOCS_NAV.map(
                                (group) => html`
                                    <section class="m-docs-sidebar__group">
                                        <h2 class="m-docs-sidebar__title">${group.section}</h2>
                                        ${group.links.map((link) => {
                                            const active = link.path === this.currentPath;
                                            return html`<a class="m-docs-sidebar__link ${active ? 'is-active' : ''}"
                                                           href="${link.path}" @click="${this.navigate}"
                                            >${link.title}</a
                                            >`;
                                        })}
                                    </section>
                                `
                        )}
                    </aside>

                    <article class="l-docs-layout__content">
                        <docs-page-view .page=${page}
                                        @docs-open-lightbox=${this.handleDocsOpenLightbox}></docs-page-view>

                        <nav class="m-docs-pagination" aria-label="Page navigation">
                            ${previous
                                    ? html`<a class="m-button" href="${previous.path}" @click="${this.navigate}">←
                                        ${previous.title}</a>`
                                    : html`<span></span>`}
                            ${next ? html`<a class="m-button m-button--primary" href="${next.path}"
                                             @click="${this.navigate}">${next.title} →</a>` : html``}
                        </nav>
                    </article>

                    <aside class="m-toc" aria-label="On this page">
                        <p class="m-toc__title">On this page</p>
                        ${page.sections.map(
                                (section) =>
                                        html`<a
                                                class="m-toc__link m-toc__link--level-${section.headingLevel ?? 2}"
                                                href="#${section.id}"
                                                @click="${(event: Event) => this.handleTableOfContentsClick(event, section.id)}"
                                        >${section.tocTitle ?? section.title}</a
                                        >`
                        )}
                    </aside>
                </div>
            </main>
        `;
    }

    protected render() {
        const lightbox = this.renderLightbox();

        if (this.isDocsRoute()) {
            return html`${lightbox}${this.renderDocs()}`;
        }

        return html`
            ${lightbox}
            <a class="m-skip-link" href="#main-content">Skip to content</a>

            <header class="l-header">
                <div class="l-shell l-header__inner">
                    <a href="#" class="m-logo" aria-label="Oatty home">
                        <img src="/icons/logo-icon.svg" alt="Oatty logo"/>
                        <span class="m-logo__wordmark">OATTY</span>
                    </a>
                    <nav class="m-nav" aria-label="Primary">
                        <a class="m-nav__link" href="#problem"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'problem')}">Problem</a>
                        <a class="m-nav__link" href="#operators"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'operators')}">Operators</a>
                        <a class="m-nav__link" href="#agents"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'agents')}">Agents</a>
                        <a class="m-nav__link" href="#features"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'features')}">Features</a>
                        <a class="m-nav__link" href="#install"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'install')}">Install</a>
                        <a class="m-nav__link m-nav__link--icon" href="https://github.com/oattyio/oatty" target="_blank"
                           rel="noopener">
                            <svg class="m-nav__icon" viewBox="0 0 24 24" aria-hidden="true">
                                <path
                                        d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7a5.44 5.44 0 0 0-1.5-3.75 5.07 5.07 0 0 0-.09-3.77s-1.18-.35-3.91 1.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1a5.07 5.07 0 0 0-.09 3.77 5.44 5.44 0 0 0-1.5 3.75c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"
                                />
                            </svg>
                            GitHub
                        </a>
                    </nav>
                    <div class="m-header-actions">
                        <a class="m-button m-button--primary" href="/docs/quick-start" @click="${this.navigate}">Start
                            Quick Start</a>
                    </div>
                </div>
            </header>

            <main id="main-content">
                <section class="l-hero">
                    <div class="l-shell">
                        <div class="l-hero__content">
                            <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP"
                                 class="m-brand-lockup m-brand-lockup--hero"/>
                            <h1 class="m-heading-4xl m-heading-balanced">Stop duct-taping curl, bash, CI YAML, and
                                vendor CLIs together.</h1>
                            <p class="m-text-lg m-text-lg--lead">
                                Ship the same workflows across vendors without rewriting glue. One consistent execution
                                surface across the APIs you operate. Keep large command surfaces available without
                                flooding agent context windows.
                            </p>
                            <div class="l-flex l-flex--center l-hero__actions">
                                <a class="m-button m-button--primary" href="#install"
                                   @click="${this.toggleHeroInstallPanel}">Install Oatty</a>
                                <a class="m-button" href="https://github.com/oattyio/oatty" target="_blank"
                                   rel="noopener">View on GitHub</a>
                                <a class="m-button" href="/docs/guides/sentry-datadog-pagerduty-playbook"
                                   @click="${this.navigate}">
                                    See cross-vendor playbook
                                </a>
                            </div>
                            <div class="m-install-teaser">
                                <span>Start using Oatty in under 60 seconds</span>
                                <code class="m-inline-code m-inline-code--body">npm install -g oatty</code>
                            </div>
                            <div class="m-install-panel ${this.isHeroInstallExpanded ? 'is-open' : ''}">
                                <pre class="m-code m-code--hero m-code--hero-shell"><code>npm install -g oatty

# Import any public OpenAPI catalog (required once)
oatty import https://petstore3.swagger.io/api/v3/openapi.json --kind catalog

# Start in TUI (recommended)
oatty

# Use CLI fallback for automation (after import)
oatty search "list pets"</code></pre>
                                <div class="l-flex l-flex--center l-flex--wrap">
                                    <a class="m-button" href="#install"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'install')}">Full
                                        install guide</a>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>

                <section class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">Demo Preview</p>
                        <h2 class="m-heading-3xl">Watch: schema import -> fuzzy search -> workflow run</h2>
                        <div class="m-video-placeholder">Short teaser loop coming soon</div>
                    </div>
                </section>

                <section class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">Choose Your Path</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">Use Oatty directly or through agents</h2>
                        <div class="l-grid l-grid--two">
                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">For Operators</h3>
                                <p class="m-card__text m-card__text--spaced-lg">
                                    Replace brittle scripts and CI glue with deterministic workflows.
                                </p>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> One workflow model across vendors</div>
                                    <div><strong class="m-checkmark">✓</strong> File-backed workflows you can diff and
                                        review
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Structured retries and conditional steps
                                    </div>
                                </div>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary" href="#operators"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'operators')}">Explore
                                        operator path</a>
                                    <a class="m-button" href="/docs/quick-start" @click="${this.navigate}">View quick
                                        start</a>
                                </div>
                            </article>
                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">For Agent-Driven Teams</h3>
                                <p class="m-card__text m-card__text--spaced-lg">
                                    Expose one safe operational surface to agents through MCP.
                                </p>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Your Agent does the heavy lifting</div>
                                    <div><strong class="m-checkmark">✓</strong> Humans review for sensitive operations
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Agent actions are reviewable, local,
                                        accessible and auditable
                                    </div>
                                </div>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary" href="#agents"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'agents')}">Explore
                                        agent path</a>
                                    <a class="m-button" href="/docs/learn/how-oatty-executes-safely"
                                       @click="${this.navigate}">Safety model</a>
                                </div>
                            </article>
                        </div>
                    </div>
                </section>

                <section id="problem" class="l-section">
                    <div class="l-shell">
                        <div class="m-card m-card--problem-hero">
                            <div class="m-content-max">
                                <p class="m-eyebrow">The Problem</p>
                                <h2 class="m-heading-3xl">The real problem is not APIs. It is coordination.</h2>
                                <p class="m-text-lg">
                                    If one workflow means juggling curl requests, jq parsing, bash loops, CI YAML,
                                    multiple
                                    vendor CLIs, and multiple auth models, you do not have an automation problem.
                                    You have a coordination problem. The same issue appears with MCP tool sprawl that
                                    overwhelms agent token budgets.
                                </p>
                            </div>
                        </div>

                        <div class="l-grid l-grid--problem-cards">
                            <div class="m-card m-card--problem-item">
                                <h3 class="m-heading-lg m-heading-spaced-sm">Inconsistent commands</h3>
                                <p class="m-card__text">Nearly identical operations with completely different naming
                                    conventions across vendors.</p>
                                <p class="m-card__resolution">Resolved in Oatty: one execution model across vendors.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <h3 class="m-heading-lg m-heading-spaced-sm">Partial coverage</h3>
                                <p class="m-card__text">Incomplete API coverage forces you back to curl or writing
                                    custom scripts.</p>
                                <p class="m-card__resolution">Resolved in Oatty: commands generated directly from
                                    OpenAPI catalogs.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <h3 class="m-heading-lg m-heading-spaced-sm">Fragmented plugins</h3>
                                <p class="m-card__text">Separate MCP servers for each vendor with even less
                                    functionality than the CLI.</p>
                                <p class="m-card__resolution">Resolved in Oatty: one MCP surface for commands and
                                    workflows.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <h3 class="m-heading-lg m-heading-spaced-sm">Brittle automation</h3>
                                <p class="m-card__text">Workflows living in opaque shell scripts that break with every
                                    vendor update.</p>
                                <p class="m-card__resolution">Resolved in Oatty: deterministic, file-backed workflows
                                    you can review.</p>
                            </div>
                        </div>

                        <div class="m-card m-card--solution">
                            <div class="m-content-max">
                                <p class="m-eyebrow">The Solution</p>
                                <h2 class="m-heading-3xl">One coherent operational surface</h2>
                                <p class="m-text-lg m-text-lg--spaced">
                                    Oatty collapses this complexity. Run one execution model across vendors with
                                    structured retries, enforced dependencies, and actionable errors.
                                    Materially less orchestration glue.
                                </p>
                                <div class="l-flex l-flex--wrap m-checklist">
                                    <span>✓ One interface</span>
                                    <span>✓ One mental model</span>
                                    <span>✓ One place to operate</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>

                <section id="comparison" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">From Glue To Workflows</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">Before and after in one view</h2>
                        <div class="l-grid l-grid--two">
                            <article class="m-card">
                                <h3 class="m-card__title">Before</h3>
                                <pre class="m-code"><code>curl ...
jq ...
while true; do
  sleep 20
done</code></pre>
                            </article>
                            <article class="m-card">
                                <h3 class="m-card__title">After</h3>
                                <pre class="m-code"><code>steps:
  - run: service:create
  - run: service:deploy
  - repeat:
      until: status == "live"</code></pre>
                            </article>
                        </div>
                        <p class="m-text-lg m-text-lg--spaced">Materially less orchestration glue.</p>
                    </div>
                </section>

                <section id="schema-rehabilitation" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">Schema Rehabilitation</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">Real-world APIs are messy. Oatty does not require
                            perfection.</h2>
                        <p class="m-text-lg m-text-lg--spaced">
                            A connected Agent can detect failures and propose corrections or build a spec from scratch
                            for legacy APIs.
                        </p>
                        <div class="l-grid l-grid--two">
                            <article class="m-card">
                                <h3 class="m-card__title">Normalize imperfect specs</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Override missing or broken operation IDs
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Fix request/response shapes and
                                        parameter types
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Add synthetic commands where vendor
                                        specs are incomplete
                                    </div>
                                </div>
                            </article>
                            <article class="m-card">
                                <h3 class="m-card__title">Patch safely and transparently</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Best with an agent connected: detect
                                        failures and propose patches
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Structured, local patches you can
                                        inspect and diff
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> No silent fixes to your operational
                                        surface
                                    </div>
                                </div>
                            </article>
                        </div>
                    </div>
                </section>

                <section id="operators" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">For Operators</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">Replace glue code with deterministic
                            workflows</h2>

                        <div class="l-grid l-grid--two">
                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">When this is you</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> CI jobs call bash, curl, and jq for one
                                        runbook
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Retries and timeouts are hand-rolled in
                                        scripts
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Vendor CLIs use different patterns and
                                        output shapes
                                    </div>
                                </div>
                                <h3 class="m-card__title">What you get on day one</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Install Oatty and connect to your Agent
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Describe your goal using natural
                                        language
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Test one reusable multi-vendor workflow
                                        with retries
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Move one brittle CI runbook into a
                                        workflow file
                                    </div>
                                </div>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary" href="#install"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'install')}">Build
                                        your first workflow</a>
                                    <a class="m-button" href="/docs/quick-start" @click="${this.navigate}">Operator
                                        quick start</a>
                                </div>
                            </article>

                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">Proof artifact: deterministic workflow</h3>
                                <p class="m-card__text">A small workflow with dependencies and retries, stored in your
                                    repo.</p>
                                <pre class="m-code"><code>steps:
  - id: create_service
    run: service:create
  - id: deploy_service
    run: service:deploy
    depends_on: [create_service]
  - id: wait_until_live
    repeat:
      run: service:status
      until: result.status == "live"</code></pre>
                                <p class="m-card__text">
                                    One model across vendors. Actionable failures. Reviewable files you can diff.
                                </p>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary"
                                       href="/docs/guides/credential-rotation-readiness-playbook"
                                       @click="${this.navigate}">
                                        See operator playbook
                                    </a>
                                    <a class="m-button" href="#features"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'features')}">Explore
                                        feature depth</a>
                                </div>
                            </article>
                        </div>
                    </div>
                </section>

                <section id="features" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">What You Get</p>
                        <h2 class="m-heading-4xl m-heading-spaced-3xl">Everything vendor CLIs should have been</h2>

                        <div class="l-stack l-stack--2xl">
                            <article class="m-card m-card--feature-hero">
                                <div class="m-feature-hero__intro">
                                    <span class="m-eyebrow m-eyebrow--xs">The TUI Experience</span>
                                    <h3 class="m-heading-3xl m-heading-spaced-md">Interactive command palette with fuzzy
                                        search</h3>
                                    <p class="m-text-lg m-text-lg--spaced">
                                        Launch <code class="m-inline-code m-inline-code--body">oatty</code> and start
                                        typing. Fuzzy search finds commands instantly. Tab for autocomplete with Value
                                        Providers that fetch live options from your APIs.
                                    </p>
                                    <div class="l-flex l-flex--wrap m-chip-list">
                                        <span class="m-chip">Ctrl+F Browser</span>
                                        <span class="m-chip">Ctrl+L Logs</span>
                                        <span class="m-chip">Ctrl+T Themes</span>
                                    </div>
                                </div>
                                <img src="/Oatty-finder.png" alt="Oatty command finder with fuzzy search"
                                     class="m-screenshot m-screenshot--hero"
                                     @click="${() => this.openLightbox('/Oatty-finder.png', 'Oatty command finder with fuzzy search')}"/>
                            </article>

                            <div class="l-grid l-grid--features-tight">
                                <article class="m-card m-card--feature-tile">
                                    <h3 class="m-heading-xl m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-library.svg" alt="" class="m-feature-title__icon"/>
                                        Library Management
                                    </h3>
                                    <p class="m-card__text m-card__text--spaced-lg">
                                        Import OpenAPI specs through the TUI. Edit metadata, configure auth,
                                        enable/disable catalogs—all without leaving the terminal.
                                    </p>
                                    <img src="/Oatty-library.png" alt="Oatty library management interface"
                                         class="m-screenshot m-screenshot--card"
                                         @click="${() => this.openLightbox('/Oatty-library.png', 'Oatty library management interface')}"/>
                                </article>

                                <article class="m-card m-card--feature-tile">
                                    <h3 class="m-heading-xl m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-workflow.svg" alt="" class="m-feature-title__icon"/>
                                        Workflow Catalog
                                    </h3>
                                    <p class="m-card__text m-card__text--spaced-lg">
                                        Browse, create, and manage workflows. Organize multi-step operations as
                                        reusable, shareable YAML definitions.
                                    </p>
                                    <img src="/Oatty-workflows-list.png" alt="Oatty workflow catalog"
                                         class="m-screenshot m-screenshot--card"
                                         @click="${() => this.openLightbox('/Oatty-workflows-list.png', 'Oatty workflow catalog')}"/>
                                </article>

                                <article class="m-card m-card--feature-tile">
                                    <h3 class="m-heading-xl m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-run.svg" alt="" class="m-feature-title__icon"/>
                                        Command Execution
                                    </h3>
                                    <p class="m-card__text m-card__text--spaced-lg">
                                        Run workflows or direct API commands with rich output. See JSON responses, logs,
                                        and execution status in real-time.
                                    </p>
                                    <img src="/Oatty-run.png" alt="Oatty command execution output"
                                         class="m-screenshot m-screenshot--card"
                                         @click="${() => this.openLightbox('/Oatty-run.png', 'Oatty command execution output')}"/>
                                </article>
                            </div>

                            <div class="l-grid l-grid--mcp-features">
                                <article class="m-card m-card--feature-tile">
                                    <h3 class="m-heading-xl m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-mcp-server.svg" alt="" class="m-feature-title__icon"/>
                                        MCP Server Mode
                                    </h3>
                                    <p class="m-card__text m-card__text--spaced-lg">
                                        Run Oatty as an MCP server. All commands and workflows exposed as tools for AI
                                        assistants—Claude, Cline, or any MCP client.
                                    </p>
                                    <img src="/Oatty-mcp-server-view.png" alt="Oatty MCP server interface"
                                         class="m-screenshot m-screenshot--card"
                                         @click="${() => this.openLightbox('/Oatty-mcp-server-view.png', 'Oatty MCP server interface')}"/>
                                </article>

                                <article class="m-card m-card--feature-tile">
                                    <h3 class="m-heading-xl m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-mcp-client.svg" alt="" class="m-feature-title__icon"/>
                                        MCP Client Mode
                                    </h3>
                                    <p class="m-card__text m-card__text--spaced-lg">
                                        Manage and execute tools from any MCP plugin. Native integrations with
                                        filesystem, GitHub, Postgres—all discoverable through the same TUI.
                                    </p>
                                    <img src="/Oatty-mcp-client.png" alt="Oatty MCP client interface"
                                         class="m-screenshot m-screenshot--card"
                                         @click="${() => this.openLightbox('/Oatty-mcp-client.png', 'Oatty MCP client interface')}"/>
                                </article>
                            </div>

                            <div class="l-grid l-grid--four">
                                <article class="m-card m-card--surface">
                                    <h3 class="m-heading-lg m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-value-providers.svg" alt=""
                                             class="m-feature-title__icon m-feature-title__icon--small"/>
                                        Value Providers
                                    </h3>
                                    <p class="m-card__text">
                                        Intelligent autocomplete that fetches live values. Provider-backed suggestions
                                        with caching and dependency resolution.
                                    </p>
                                </article>

                                <article class="m-card m-card--surface">
                                    <h3 class="m-heading-lg m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-command-browser.svg" alt=""
                                             class="m-feature-title__icon m-feature-title__icon--small"/>
                                        Command Browser
                                    </h3>
                                    <p class="m-card__text">
                                        Explore your entire API surface with searchable, categorized views. See methods,
                                        summaries, and details before execution.
                                    </p>
                                </article>

                                <article class="m-card m-card--surface">
                                    <h3 class="m-heading-lg m-feature-title m-heading-spaced-sm">
                                        <img src="/icons/icon-rich-logging.svg" alt=""
                                             class="m-feature-title__icon m-feature-title__icon--small"/>
                                        Rich Logging
                                    </h3>
                                    <p class="m-card__text">
                                        Searchable logs panel with syntax highlighting. Toggle with Ctrl+L. See
                                        execution history and debug workflows.
                                    </p>
                                </article>
                            </div>

                            <article class="m-card m-card--schema">
                                <div class="m-content-max">
                                    <h3 class="m-heading-2xl m-heading-spaced-md">Schema-Driven Everything</h3>
                                    <p class="m-text-lg m-text-lg--spaced">
                                        Commands are automatically derived from OpenAPI documents. Full API coverage
                                        without waiting for vendors. If it's in the spec, it's available immediately.
                                    </p>
                                    <div class="l-flex l-flex--wrap m-schema-list">
                                        <div><strong class="m-checkmark">✓</strong> Auto-sync with API changes</div>
                                        <div><strong class="m-checkmark">✓</strong> Coverage tracks your OpenAPI spec
                                        </div>
                                        <div><strong class="m-checkmark">✓</strong> MCP tool integration</div>
                                    </div>
                                </div>
                            </article>
                        </div>
                    </div>
                </section>

                <section id="agents" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">For Agent-Driven Teams</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">One MCP Surface. Not Seven.</h2>
                        <p class="m-text-lg m-text-lg--spaced">
                            Avoid context-window saturation from oversized MCP tool lists. Oatty keeps a large command
                            surface available while
                            narrowing execution through fuzzy discovery and targeted invocation.
                        </p>
                        <div class="l-grid l-grid--three l-grid--section-gap">
                            <article class="m-card">
                                <h3 class="m-card__title">Deterministic Execution Layer</h3>
                                <p class="m-card__text">Route agent intent through one controlled execution model across
                                    vendors.</p>
                            </article>
                            <article class="m-card">
                                <h3 class="m-card__title">Coherent, Auditable Surface</h3>
                                <p class="m-card__text">Keep writes reviewable with previews and explicit approval
                                    checkpoints.</p>
                            </article>
                            <article class="m-card">
                                <h3 class="m-card__title">Schema Normalization Built In</h3>
                                <p class="m-card__text">Patch spec inconsistencies locally so agent operations stay
                                    stable.</p>
                            </article>
                        </div>
                        <div class="l-grid l-grid--two">
                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">When this is you</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> You use multiple MCP servers and CLIs
                                        across
                                        vendors
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Large tool lists blow up prompts and
                                        force context switching
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> You need guardrails around sensitive
                                        writes
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Agent output is hard to standardize and
                                        reuse
                                    </div>
                                </div>
                                <h3 class="m-card__title">What you get on day one</h3>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Expose catalogs and workflows through
                                        Oatty MCP
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Route agent requests through fuzzy
                                        search instead of huge static tool dumps
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Keep humans in review loop for risky
                                        changes
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Agent-proposed schema fixes stay local,
                                        reviewable, and auditable
                                        patches
                                    </div>
                                </div>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary"
                                       href="/docs/guides/sentry-datadog-pagerduty-playbook" @click="${this.navigate}">
                                        Run cross-vendor playbook
                                    </a>
                                    <a class="m-button" href="/docs/learn/how-oatty-executes-safely"
                                       @click="${this.navigate}">Execution safety model</a>
                                </div>
                            </article>
                            <article class="m-card m-card--persona">
                                <h3 class="m-card__title">Proof artifact: agent flow with review gates</h3>
                                <p class="m-card__text">A minimal sequence for agent speed with explicit human
                                    control.</p>
                                <pre class="m-code"><code>1. Agent searches intent, not an overstuffed context of MCP tools
2. Oatty resolves a targeted command set via fuzzy discovery
3. Oatty renders a structured execution preview
4. Human reviews and approves sensitive writes</code></pre>
                                <div class="l-stack l-stack--sm">
                                    <div><strong class="m-checkmark">✓</strong> Cross-vendor API discovery</div>
                                    <div><strong class="m-checkmark">✓</strong> Structured previews before execution
                                    </div>
                                    <div><strong class="m-checkmark">✓</strong> Explicit approval checkpoints</div>
                                </div>
                                <div class="l-flex l-flex--wrap m-persona-card__actions">
                                    <a class="m-button m-button--primary" href="/docs/guides/sentry-bootstrap"
                                       @click="${this.navigate}">Read Sentry bootstrap guide</a>
                                    <a class="m-button" href="#features"
                                       @click="${(event: Event) => this.smoothScrollToSection(event, 'features')}">Explore
                                        feature depth</a>
                                </div>
                            </article>
                        </div>
                        <p class="m-text-lg m-heading-centered m-heading-spaced-md">Agents talk to Oatty. Oatty talks to
                            vendors.</p>
                    </div>
                </section>

                <section id="install" class="l-section l-section--install">
                    <div class="l-shell">
                        <h2 class="m-heading-4xl m-heading-centered m-heading-spaced-xl">Getting Started</h2>
                        <p class="m-text-lg m-heading-centered">
                            Early release, shipping fast: Oatty is currently in the <code
                                class="m-inline-code m-inline-code--body">v0.1</code> line with active MCP and workflow
                            docs.
                        </p>
                        <div class="l-grid l-grid--two">
                            <div class="m-card">
                                <h3 class="m-card__title">Install via npm</h3>
                                <pre class="m-code"><code>npm install -g oatty
oatty --help</code></pre>
                                <p class="m-card__text">
                                    Automatically downloads the matching prebuilt binary for your platform.
                                </p>
                            </div>
                            <div class="m-card">
                                <h3 class="m-card__title">Install from source</h3>
                                <pre class="m-code"><code>cargo build --release
./target/release/oatty</code></pre>
                                <p class="m-card__text">
                                    Build from source with Rust 1.93+. Full control over compilation.
                                </p>
                            </div>
                        </div>
                        <div class="m-card m-card--spaced-top">
                            <h3 class="m-card__title">Quick Start</h3>
                            <pre class="m-code"><code># Start interactive TUI
oatty

# Import an OpenAPI catalog (path or URL)
oatty import ./openapi.json --kind catalog
# oatty import https://example.com/openapi.json --kind catalog

# Search for commands
oatty search "create order"

# Run a workflow
oatty workflow list
oatty workflow run deploy --input env=staging</code></pre>
                            <p class="m-card__text">Use the guided flow in <a href="/docs/quick-start"
                                                                              @click="${this.navigate}">Quick Start
                                docs</a>.</p>
                        </div>
                    </div>
                </section>

                <section class="l-section">
                    <div class="l-shell">
                        <h2 class="m-heading-4xl m-heading-spaced-xl">Architecture</h2>
                        <div class="l-grid l-grid--three">
                            <article class="m-card">
                                <div class="m-step-index">01</div>
                                <h3 class="m-card__title">Registry</h3>
                                <p class="m-card__text">
                                    Loads catalog manifests derived from OpenAPI documents. Stores configuration in
                                    <code class="m-inline-code m-inline-code--accent">~/.config/oatty/</code>
                                </p>
                            </article>
                            <article class="m-card">
                                <div class="m-step-index">02</div>
                                <h3 class="m-card__title">CLI / TUI</h3>
                                <p class="m-card__text">
                                    Builds command tree from registry. CLI routes to HTTP/MCP execution. TUI provides
                                    interactive search and composition.
                                </p>
                            </article>
                            <article class="m-card">
                                <div class="m-step-index">03</div>
                                <h3 class="m-card__title">MCP Engine</h3>
                                <p class="m-card__text">
                                    Manages MCP plugin lifecycles. Injects tool commands into the registry at runtime
                                    for seamless integration.
                                </p>
                            </article>
                        </div>
                    </div>
                </section>

                <section class="l-section">
                    <div class="l-shell">
                        <div class="m-card m-card--solution">
                            <div class="m-content-max">
                                <h2 class="m-heading-3xl">Less glue. Less guessing. Less 2am debugging.</h2>
                                <p class="m-text-lg m-text-lg--spaced">
                                    Use one execution model for direct CLI operation and agent-driven workflows.
                                    Keep every change reviewable, local, and auditable.
                                </p>
                                <div class="l-flex l-flex--wrap">
                                    <a class="m-button m-button--primary"
                                       href="/docs/guides/sentry-datadog-pagerduty-playbook" @click="${this.navigate}">
                                        See a real workflow guide
                                    </a>
                                    <a class="m-button" href="/docs/quick-start" @click="${this.navigate}">Start quick
                                        start</a>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>

                <footer class="l-footer">
                    <div class="l-shell">
                        <div class="l-footer__top">
                            <div>
                                <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP"
                                     class="m-brand-lockup m-brand-lockup--footer"/>
                            </div>
                            <nav class="m-footer__links">
                                <a href="https://github.com/oattyio/oatty" target="_blank" rel="noopener"
                                   class="m-footer__link">GitHub</a>
                                <a href="/docs/quick-start" @click="${this.navigate}" class="m-footer__link">Documentation</a>
                                <a href="https://github.com/oattyio/oatty/discussions" target="_blank" rel="noopener"
                                   class="m-footer__link">Community</a>
                                <a href="https://github.com/oattyio/oatty/issues" target="_blank" rel="noopener"
                                   class="m-footer__link">Issues</a>
                            </nav>
                        </div>
                        <div class="l-footer__bottom">
                            <p class="m-footer__meta">MIT OR Apache-2.0 License • Built with Rust</p>
                        </div>
                    </div>
                </footer>
            </main>
        `;
    }
}

customElements.define('oatty-site-app', OattySiteApp);
