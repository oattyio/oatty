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

const DEFAULT_PAGE_TITLE = 'Oatty | One CLI for Every API';
const DEFAULT_PAGE_DESCRIPTION = 'Schema-driven command discovery, interactive terminal UI, and extension via MCP. Stop juggling vendor CLIs with Oatty - one coherent operational surface for all your APIs.';
const SITE_ORIGIN = 'https://oatty.io';

const DOCS_NAV = [
    {
        section: 'Get Started',
        links: [{title: 'Quick Start', path: '/docs/quick-start'}],
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
                                        >${section.title}</a
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
                        <a class="m-nav__link" href="#principles"
                           @click="${(event: Event) => this.smoothScrollToSection(event, 'principles')}">Principles</a>
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
                            <h1 class="m-heading-4xl m-heading-balanced">Your Unified Command Surface</h1>
                            <p class="m-text-lg m-text-lg--lead">
                                A beautiful TUI with schema-driven discovery, intelligent autocomplete, and workflow
                                automation. Stop juggling a dozen vendor CLIs.
                            </p>
                            <div class="l-flex l-flex--center l-hero__actions">
                                <a class="m-button m-button--primary" href="#install">Get Started</a>
                                <a class="m-button" href="https://github.com/oattyio/oatty" target="_blank"
                                   rel="noopener">View on GitHub</a>
                            </div>
                            <pre class="m-code m-code--hero m-code--hero-shell"><code>npm install -g oatty

# Import a public OpenAPI catalog (required once)
oatty import https://petstore3.swagger.io/api/v3/openapi.json --kind catalog

# Start in TUI (recommended)
oatty

# Use CLI fallback for automation (after import)
oatty search "list pets"</code></pre>
                        </div>
                    </div>
                </section>

                <section id="problem" class="l-section">
                    <div class="l-shell">
                        <div class="m-card m-card--problem-hero">
                            <div class="m-content-max">
                                <p class="m-eyebrow">The Problem</p>
                                <h2 class="m-heading-3xl">Vendor CLIs are fragmented, incomplete, and inconsistent</h2>
                                <p class="m-text-lg">
                                    Modern APIs are powerful and well-documented, but the developer experience is
                                    broken. You're forced to juggle a dozen different CLIs, each with partial coverage
                                    and different conventions.
                                </p>
                            </div>
                        </div>

                        <div class="l-grid l-grid--problem-cards">
                            <div class="m-card m-card--problem-item">
                                <div class="m-icon-chip m-icon-chip--problem">
                                    <img src="/icons/icon-problem-inconsistent.svg" alt="" class="m-icon-size-sm"/>
                                </div>
                                <h3 class="m-heading-lg m-heading-spaced-sm">Inconsistent commands</h3>
                                <p class="m-card__text">Nearly identical operations with completely different naming
                                    conventions across vendors.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <div class="m-icon-chip m-icon-chip--problem">
                                    <img src="/icons/icon-problem-coverage-gap.svg" alt="" class="m-icon-size-sm"/>
                                </div>
                                <h3 class="m-heading-lg m-heading-spaced-sm">Partial coverage</h3>
                                <p class="m-card__text">Incomplete API coverage forces you back to curl or writing
                                    custom scripts.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <div class="m-icon-chip m-icon-chip--problem">
                                    <img src="/icons/icon-plugin-fragmentation.svg" alt="" class="m-icon-size-sm"/>
                                </div>
                                <h3 class="m-heading-lg m-heading-spaced-sm">Fragmented plugins</h3>
                                <p class="m-card__text">Separate MCP servers for each vendor with even less
                                    functionality than the CLI.</p>
                            </div>

                            <div class="m-card m-card--problem-item">
                                <div class="m-icon-chip m-icon-chip--problem">
                                    <img src="/icons/icon-brittle-automation.svg" alt="" class="m-icon-size-sm"/>
                                </div>
                                <h3 class="m-heading-lg m-heading-spaced-sm">Brittle automation</h3>
                                <p class="m-card__text">Workflows living in opaque shell scripts that break with every
                                    vendor update.</p>
                            </div>
                        </div>

                        <div class="m-card m-card--solution">
                            <div class="m-content-max">
                                <p class="m-eyebrow">The Solution</p>
                                <h2 class="m-heading-3xl">One coherent operational surface</h2>
                                <p class="m-text-lg m-text-lg--spaced">
                                    Oatty collapses this complexity. Turn OpenAPI documents into runnable commands,
                                    explore them in a beautiful TUI, and automate with workflows-all through one
                                    consistent interface.
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

                <section id="principles" class="l-section">
                    <div class="l-shell">
                        <p class="m-eyebrow">Design Philosophy</p>
                        <h2 class="m-heading-4xl m-heading-spaced-2xl">Built for the terminal. Designed for humans.</h2>

                        <div class="l-grid l-grid--principles">
                            <article class="m-card m-card--principle-hero">
                                <div>
                                    <div class="m-icon-panel">
                                        <img src="/icons/icon-discoverability.svg" alt="" class="m-icon-fill"/>
                                    </div>
                                    <h3 class="m-heading-2xl m-heading-spaced-md">Discoverability</h3>
                                    <p class="m-text-lg">
                                        You should never need to memorize commands. Guided UI with inline search,
                                        contextual hints, and discoverable keybindings. If the API supports it, you can
                                        find it.
                                    </p>
                                </div>
                                <div class="m-terminal-snippet">
                                    <div class="m-terminal-snippet__title">// Type to search</div>
                                    <div>oatty<span class="m-text-accent">▊</span></div>
                                    <div class="m-terminal-snippet__results">
                                        <div class="m-terminal-snippet__item">→ apps create</div>
                                        <div class="m-terminal-snippet__item">→ apps list</div>
                                        <div class="m-terminal-snippet__item">→ databases create</div>
                                    </div>
                                </div>
                            </article>

                            <article class="m-card m-card--elevated">
                                <div class="m-icon-box">
                                    <img src="/icons/icon-simplicity.svg" alt="" class="m-icon-fill"/>
                                </div>
                                <h3 class="m-heading-xl m-heading-spaced-sm">Simplicity</h3>
                                <p class="m-card__text">
                                    Each screen does one thing, clearly. Search on top, results in center, details on
                                    right. No clutter, no overloaded views, no hidden modes.
                                </p>
                            </article>

                            <article class="m-card m-card--elevated">
                                <div class="m-icon-box">
                                    <img src="/icons/icon-speed.svg" alt="" class="m-icon-fill"/>
                                </div>
                                <h3 class="m-heading-xl m-heading-spaced-sm">Speed</h3>
                                <p class="m-card__text">
                                    Designed for real work, not demos. Command palette with history navigation and instant autocomplete.
                                </p>
                            </article>

                            <article class="m-card m-card--elevated">
                                <div class="m-icon-box">
                                    <img src="/icons/icon-consistency.svg" alt="" class="m-icon-fill"/>
                                </div>
                                <h3 class="m-heading-xl m-heading-spaced-sm">Consistency</h3>
                                <p class="m-card__text">
                                    Workflows behave like commands. The same search, execution, and logging interface
                                    across HTTP commands, MCP tools, and workflows.
                                </p>
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
                                        <div><strong class="m-checkmark">✓</strong> Coverage tracks your OpenAPI spec</div>
                                        <div><strong class="m-checkmark">✓</strong> MCP tool integration</div>
                                    </div>
                                </div>
                            </article>
                        </div>
                    </div>
                </section>

                <section id="install" class="l-section l-section--install">
                    <div class="l-shell">
                        <h2 class="m-heading-4xl m-heading-centered m-heading-spaced-xl">Getting Started</h2>
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
