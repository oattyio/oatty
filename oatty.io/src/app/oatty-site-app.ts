import { LitElement, html } from 'lit';

import { applyConstructibleStyles } from '../styles/sheets/constructible-style';
import { baseStyle } from '../styles/sheets/base-sheet';
import { layoutStyle } from '../styles/sheets/layout-sheet';
import { moduleStyle } from '../styles/sheets/module-sheet';
import { stateStyle } from '../styles/sheets/state-sheet';
import { themeStyle } from '../styles/sheets/theme-sheet';

export class OattySiteApp extends LitElement {
  protected createRenderRoot(): ShadowRoot {
    return this.attachShadow({ mode: 'open' });
  }

  connectedCallback(): void {
    super.connectedCallback();

    if (this.shadowRoot) {
      applyConstructibleStyles(this.shadowRoot, [themeStyle, baseStyle, layoutStyle, moduleStyle, stateStyle]);
    }
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

  protected render() {
    return html`
      <a class="m-skip-link" href="#main-content">Skip to content</a>

      <!-- Lightbox for screenshots -->
      <div class="m-lightbox" @click="${this.closeLightbox}">
        <div class="m-lightbox__close" aria-label="Close lightbox">✕</div>
        <img src="" alt="" @click="${(e: Event) => e.stopPropagation()}" />
      </div>

      <header class="l-header">
        <div class="l-shell l-header__inner">
          <a href="#" class="m-logo" aria-label="Oatty home">
            <img src="/logo-icon.svg" alt="Oatty logo" style="width: 2rem; height: 2rem;" />
            <span style="font-size: var(--font-size-lg); font-weight: 700; letter-spacing: 0.05em;">OATTY</span>
          </a>
          <nav class="m-nav" aria-label="Primary">
            <a class="m-nav__link" href="#problem">Problem</a>
            <a class="m-nav__link" href="#principles">Principles</a>
            <a class="m-nav__link" href="#features">Features</a>
            <a class="m-nav__link" href="#install">Install</a>
          </nav>
          <div class="m-header-actions">
            <a class="m-button m-button--primary" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">
              Get Started
            </a>
          </div>
        </div>
      </header>

      <main id="main-content">
        <!-- Hero Section -->
        <section class="l-hero">
          <div class="l-shell">
            <div class="l-hero__content">
              <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP" style="width: min(600px, 90%); height: auto; margin: 0 auto var(--space-2xl); display: block; filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.3));" />
              <h1 style="font-size: var(--font-size-4xl); font-weight: 700; line-height: var(--line-height-tight); margin: 0; text-wrap: balance;">
                Your Unified Command Surface
              </h1>
              <p style="font-size: var(--font-size-xl); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin: var(--space-lg) 0;">
                A beautiful TUI with schema-driven discovery, intelligent autocomplete, and workflow automation. Stop juggling a dozen vendor CLIs.
              </p>
              <div class="l-flex" style="justify-content: center; margin-top: var(--space-xl);">
                <a class="m-button m-button--primary" href="#install">Get Started</a>
                <a class="m-button" href="https://github.com/oattyio/oatty" target="_blank" rel="noopener">View on GitHub</a>
              </div>
              <pre class="m-code m-code--hero" style="max-width: 600px; margin: var(--space-2xl) auto 0;"><code>npm install -g oatty

# Launch interactive TUI (recommended)
oatty

# Or use CLI mode for scripting
oatty apps create --name myapp
oatty workflow run deploy --input env=prod</code></pre>
            </div>
          </div>
        </section>

        <!-- Problem Section - Asymmetric Layout -->
        <section id="problem" class="l-section">
          <div class="l-shell">
            <!-- Large hero card -->
            <div class="m-card" style="background: var(--gradient-brand-subtle); border: 1px solid var(--color-accent); margin-bottom: var(--space-2xl); padding: var(--space-3xl);">
              <div style="max-width: 65ch;">
                <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">The Problem</p>
                <h2 style="font-size: var(--font-size-3xl); font-weight: 700; line-height: var(--line-height-tight); margin-bottom: var(--space-lg);">
                  Vendor CLIs are fragmented, incomplete, and inconsistent
                </h2>
                <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed);">
                  Modern APIs are powerful and well-documented, but the developer experience is broken. You're forced to juggle a dozen different CLIs, each with partial coverage and different conventions.
                </p>
              </div>
            </div>

            <!-- Grid of problem cards -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr)); gap: var(--space-lg); margin-bottom: var(--space-3xl);">
              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-problem-inconsistent.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Inconsistent commands</h3>
                <p class="m-card__text">Nearly identical operations with completely different naming conventions across vendors.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-problem-coverage-gap.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Partial coverage</h3>
                <p class="m-card__text">Incomplete API coverage forces you back to curl or writing custom scripts.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-plugin-fragmentation.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Fragmented plugins</h3>
                <p class="m-card__text">Separate MCP servers for each vendor with even less functionality than the CLI.</p>
              </div>

              <div class="m-card" style="background: var(--color-background-alt);">
                <div style="width: 2.5rem; height: 2.5rem; border-radius: var(--radius-md); background: rgba(191, 97, 106, 0.15); display: grid; place-items: center; margin-bottom: var(--space-md);">
                  <img src="/icon-brittle-automation.svg" alt="" style="width: 1.25rem; height: 1.25rem;" />
                </div>
                <h3 style="font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--space-sm);">Brittle automation</h3>
                <p class="m-card__text">Workflows living in opaque shell scripts that break with every vendor update.</p>
              </div>
            </div>

            <!-- Solution card -->
            <div class="m-card" style="background: linear-gradient(135deg, rgba(136, 192, 208, 0.1) 0%, rgba(129, 161, 193, 0.05) 100%); border: 1px solid var(--color-accent); padding: var(--space-3xl);">
              <div style="max-width: 65ch;">
                <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">The Solution</p>
                <h2 style="font-size: var(--font-size-3xl); font-weight: 700; line-height: var(--line-height-tight); margin-bottom: var(--space-lg);">
                  One coherent operational surface
                </h2>
                <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                  Oatty collapses this complexity. Turn OpenAPI documents into runnable commands, explore them in a beautiful TUI, and automate with workflows—all through one consistent interface.
                </p>
                <div style="display: flex; gap: var(--space-xl); flex-wrap: wrap; color: var(--color-accent); font-weight: 600;">
                  <span>✓ One interface</span>
                  <span>✓ One mental model</span>
                  <span>✓ One place to operate</span>
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- Core Principles - Magazine Layout -->
        <section id="principles" class="l-section">
          <div class="l-shell">
            <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">Design Philosophy</p>
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-2xl);">
              Built for the terminal. Designed for humans.
            </h2>

            <!-- 2x2 Grid with varying sizes -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: var(--space-lg);">
              <!-- Large feature card - Discoverability -->
              <article class="m-card" style="grid-column: span 2; background: var(--gradient-brand-subtle); padding: var(--space-3xl); display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2xl); align-items: center;">
                <div>
                  <div style="width: 5rem; height: 5rem; border-radius: var(--radius-xl); background: rgba(136, 192, 208, 0.2); display: grid; place-items: center; padding: 1rem; margin-bottom: var(--space-lg);">
                    <img src="/icon-discoverability.svg" alt="" style="width: 100%; height: 100%;" />
                  </div>
                  <h3 style="font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-md);">Discoverability</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed);">
                    You should never need to memorize commands. Guided UI with inline search, contextual hints, and discoverable keybindings. If the API supports it, you can find it.
                  </p>
                </div>
                <div style="background: var(--color-background-alt); border-radius: var(--radius-lg); padding: var(--space-lg); font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-text-secondary);">
                  <div style="color: var(--color-accent); margin-bottom: var(--space-sm);">// Type to search</div>
                  <div>oatty<span style="color: var(--color-accent);">▊</span></div>
                  <div style="margin-top: var(--space-md); padding: var(--space-sm); background: rgba(136, 192, 208, 0.1); border-radius: var(--radius-sm);">
                    <div style="opacity: 0.7;">→ apps create</div>
                    <div style="opacity: 0.7;">→ apps list</div>
                    <div style="opacity: 0.7;">→ databases create</div>
                  </div>
                </div>
              </article>

              <!-- Compact cards -->
              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-simplicity.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Simplicity</h3>
                <p class="m-card__text">
                  Each screen does one thing, clearly. Search on top, results in center, details on right. No clutter, no overloaded views, no hidden modes.
                </p>
              </article>

              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-speed.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Speed</h3>
                <p class="m-card__text">
                  Designed for real work, not demos. Command prompt with <code style="font-family: var(--font-mono); background: var(--color-background-alt); padding: 0.125rem 0.375rem; border-radius: var(--radius-sm);">:</code> prefix, history navigation, instant autocomplete.
                </p>
              </article>

              <article class="m-card" style="background: var(--color-elevated);">
                <div style="width: 4rem; height: 4rem; border-radius: var(--radius-lg); background: var(--gradient-brand-subtle); display: grid; place-items: center; padding: 0.75rem; margin-bottom: var(--space-md);">
                  <img src="/icon-consistency.svg" alt="" style="width: 100%; height: 100%;" />
                </div>
                <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm);">Consistency</h3>
                <p class="m-card__text">
                  Workflows behave like commands. The same search, execution, and logging interface across HTTP commands, MCP tools, and workflows.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Features - Staggered Layout -->
        <section id="features" class="l-section">
          <div class="l-shell">
            <p style="font-size: var(--font-size-sm); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); margin-bottom: var(--space-md); font-weight: 700;">What You Get</p>
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-3xl);">
              Everything vendor CLIs should have been
            </h2>

            <!-- Staggered feature cards -->
            <div style="display: flex; flex-direction: column; gap: var(--space-2xl);">

              <!-- TUI First - Large hero with screenshot -->
              <article class="m-card" style="background: var(--gradient-brand-subtle); padding: var(--space-3xl); border: 1px solid var(--color-accent); overflow: hidden;">
                <div style="margin-bottom: var(--space-2xl);">
                  <span style="font-size: var(--font-size-xs); text-transform: uppercase; letter-spacing: 0.1em; color: var(--color-accent); font-weight: 700;">The TUI Experience</span>
                  <h3 style="font-size: var(--font-size-3xl); font-weight: 700; margin: var(--space-md) 0;">Interactive command palette with fuzzy search</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                    Launch <code style="font-family: var(--font-mono); background: var(--color-background-alt); padding: 0.25rem 0.5rem; border-radius: var(--radius-sm);">oatty</code> and start typing. Fuzzy search finds commands instantly. Tab for autocomplete with Value Providers that fetch live options from your APIs.
                  </p>
                  <div style="display: flex; gap: var(--space-md); flex-wrap: wrap;">
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Ctrl+F Browser</span>
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Ctrl+L Logs</span>
                    <span style="padding: 0.375rem 0.75rem; background: var(--color-elevated); border-radius: var(--radius-full); font-size: var(--font-size-sm);">Ctrl+T Themes</span>
                  </div>
                </div>
                <img src="/Oatty-finder.png" alt="Oatty command finder with fuzzy search" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-lg); border: 1px solid var(--color-divider); box-shadow: var(--shadow-xl);" @click="${() => this.openLightbox('/Oatty-finder.png', 'Oatty command finder with fuzzy search')}" />
              </article>

              <!-- Three column row with screenshots - Core Features -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-library.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Library Management
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Import OpenAPI specs through the TUI. Edit metadata, configure auth, enable/disable catalogs—all without leaving the terminal.
                  </p>
                  <img src="/Oatty-library.png" alt="Oatty library management interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${() => this.openLightbox('/Oatty-library.png', 'Oatty library management interface')}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-workflow.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Workflow Catalog
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Browse, create, and manage workflows. Organize multi-step operations as reusable, shareable YAML definitions.
                  </p>
                  <img src="/Oatty-workflows-list.png" alt="Oatty workflow catalog" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${() => this.openLightbox('/Oatty-workflows-list.png', 'Oatty workflow catalog')}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-run.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    Command Execution
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Run workflows or direct API commands with rich output. See JSON responses, logs, and execution status in real-time.
                  </p>
                  <img src="/Oatty-run.png" alt="Oatty command execution output" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${() => this.openLightbox('/Oatty-run.png', 'Oatty command execution output')}" />
                </article>
              </div>

              <!-- Two column row with screenshots - MCP Integration -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 400px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-mcp-server.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    MCP Server Mode
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Run Oatty as an MCP server. All commands and workflows exposed as tools for AI assistants—Claude, Cline, or any MCP client.
                  </p>
                  <img src="/Oatty-mcp-server.png" alt="Oatty MCP server interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${() => this.openLightbox('/Oatty-mcp-server.png', 'Oatty MCP server interface')}" />
                </article>

                <article class="m-card" style="background: var(--color-elevated); overflow: hidden;">
                  <h3 style="font-size: var(--font-size-xl); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-mcp-client.svg" alt="" style="width: 1.5rem; height: 1.5rem;" />
                    MCP Client Mode
                  </h3>
                  <p class="m-card__text" style="margin-bottom: var(--space-lg);">
                    Manage and execute tools from any MCP plugin. Native integrations with filesystem, GitHub, Postgres—all discoverable through the same TUI.
                  </p>
                  <img src="/Oatty-mcp-client.png" alt="Oatty MCP client interface" class="m-screenshot" style="width: 100%; height: auto; border-radius: var(--radius-md); border: 1px solid var(--color-divider); margin-top: auto;" @click="${() => this.openLightbox('/Oatty-mcp-client.png', 'Oatty MCP client interface')}" />
                </article>
              </div>

              <!-- Three column row -->
              <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr)); gap: var(--space-lg);">
                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-value-providers.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Value Providers
                  </h3>
                  <p class="m-card__text">
                    Intelligent autocomplete that fetches live values. Provider-backed suggestions with caching and dependency resolution.
                  </p>
                </article>

                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-command-browser.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Command Browser
                  </h3>
                  <p class="m-card__text">
                    Explore your entire API surface with searchable, categorized views. See methods, summaries, and details before execution.
                  </p>
                </article>

                <article class="m-card" style="background: var(--color-surface);">
                  <h3 style="font-size: var(--font-size-lg); font-weight: 700; margin-bottom: var(--space-sm); display: flex; align-items: center; gap: var(--space-sm);">
                    <img src="/icon-rich-logging.svg" alt="" style="width: 1rem; height: 1rem;" />
                    Rich Logging
                  </h3>
                  <p class="m-card__text">
                    Searchable logs panel with syntax highlighting. Toggle with Ctrl+L. See execution history and debug workflows.
                  </p>
                </article>
              </div>

              <!-- Full width feature -->
              <article class="m-card" style="background: var(--color-elevated); padding: var(--space-3xl);">
                <div style="max-width: 65ch;">
                  <h3 style="font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-md);">Schema-Driven Everything</h3>
                  <p style="font-size: var(--font-size-lg); color: var(--color-text-secondary); line-height: var(--line-height-relaxed); margin-bottom: var(--space-lg);">
                    Commands are automatically derived from OpenAPI documents. Full API coverage without waiting for vendors. If it's in the spec, it's available immediately.
                  </p>
                  <div style="display: flex; gap: var(--space-xl); flex-wrap: wrap; color: var(--color-text-secondary); font-size: var(--font-size-sm);">
                    <div><strong style="color: var(--color-accent);">✓</strong> Auto-sync with API changes</div>
                    <div><strong style="color: var(--color-accent);">✓</strong> 100% coverage guaranteed</div>
                    <div><strong style="color: var(--color-accent);">✓</strong> MCP tool integration</div>
                  </div>
                </div>
              </article>

            </div>
          </div>
        </section>

        <!-- Installation -->
        <section id="install" class="l-section" style="background: var(--gradient-brand-subtle); border-radius: var(--radius-xl); padding: var(--space-4xl) 0;">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl); text-align: center;">
              Getting Started
            </h2>
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
            <div class="m-card" style="margin-top: var(--space-xl);">
              <h3 class="m-card__title">Quick Start</h3>
              <pre class="m-code"><code># Start interactive TUI
oatty

# Import an OpenAPI catalog
oatty catalog import ./schemas/your-api.json

# Search for commands
oatty search "create order"

# Run a workflow
oatty workflow list
oatty workflow run deploy --input env=staging</code></pre>
            </div>
          </div>
        </section>

        <!-- Architecture -->
        <section class="l-section">
          <div class="l-shell">
            <h2 style="font-size: var(--font-size-4xl); font-weight: 700; margin: 0 0 var(--space-xl);">
              Architecture
            </h2>
            <div class="l-grid l-grid--three">
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  01
                </div>
                <h3 class="m-card__title">Registry</h3>
                <p class="m-card__text">
                  Loads catalog manifests derived from OpenAPI documents. Stores configuration in <code style="font-family: var(--font-mono); font-size: var(--font-size-sm); color: var(--color-accent);">~/.config/oatty/</code>
                </p>
              </article>
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  02
                </div>
                <h3 class="m-card__title">CLI / TUI</h3>
                <p class="m-card__text">
                  Builds command tree from registry. CLI routes to HTTP/MCP execution. TUI provides interactive search and composition.
                </p>
              </article>
              <article class="m-card">
                <div style="color: var(--color-accent); font-size: var(--font-size-2xl); font-weight: 700; margin-bottom: var(--space-sm);">
                  03
                </div>
                <h3 class="m-card__title">MCP Engine</h3>
                <p class="m-card__text">
                  Manages MCP plugin lifecycles. Injects tool commands into the registry at runtime for seamless integration.
                </p>
              </article>
            </div>
          </div>
        </section>

        <!-- Footer -->
        <footer class="l-footer">
          <div class="l-shell">
            <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: var(--space-xl);">
              <div>
                <img src="/logo-lockup.svg" alt="Oatty - Schema-driven CLI+TUI+MCP" style="width: min(280px, 100%); height: auto; margin-bottom: var(--space-md); filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.2));" />
              </div>
              <nav style="display: flex; gap: var(--space-xl); flex-wrap: wrap;">
                <a href="https://github.com/oattyio/oatty" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  GitHub
                </a>
                <a href="https://github.com/oattyio/oatty#readme" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Documentation
                </a>
                <a href="https://github.com/oattyio/oatty/discussions" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Community
                </a>
                <a href="https://github.com/oattyio/oatty/issues" target="_blank" rel="noopener" style="color: var(--color-text-secondary); text-decoration: none; font-size: var(--font-size-sm); transition: color var(--transition-fast);">
                  Issues
                </a>
              </nav>
            </div>
            <div style="margin-top: var(--space-xl); padding-top: var(--space-xl); border-top: 1px solid var(--color-divider); text-align: center;">
              <p style="color: var(--color-text-muted); font-size: var(--font-size-sm); margin: 0;">
                MIT OR Apache-2.0 License • Built with Rust
              </p>
            </div>
          </div>
        </footer>
      </main>
    `;
  }
}

customElements.define('oatty-site-app', OattySiteApp);
