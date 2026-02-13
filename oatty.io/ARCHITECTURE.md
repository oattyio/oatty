# Oatty.io Architecture

This document captures the current site architecture and the styling/component decisions made to keep implementation scalable as docs pages are added.

## Goals

- Keep the site fast and simple: Lit web components + Vite.
- Reuse styles through constructible stylesheets.
- Prevent large future refactors by establishing a clear layering model now.
- Support a docs-first information architecture (`/docs/...`) without introducing framework lock-in.

## Runtime and Rendering Model

- Entrypoint: `src/main.ts`
- Root component: `src/app/oatty-site-app.ts`
- UI is rendered in Shadow DOM via Lit (`LitElement`).
- Route handling is currently client-side in `oatty-site-app`:
  - Marketing page for `/`
  - Docs views for `/docs/...`

## Component Strategy

Current state:

- `oatty-site-app` owns routing and top-level rendering.
- `theme-switcher` is an example reusable widget component.

Near-term structure (incremental):

1. Keep routing and page state in `oatty-site-app`.
2. Extract reusable docs widgets first:
   - docs shell
   - left nav
   - "What you'll learn" card
   - right TOC
   - prev/next pager
3. Extract page views next (`quick-start-view`, `learn-*`, `guides-*`).

This follows a practical split of `widget/component/view` without blocking delivery.

## Constructible Stylesheet System

### Design intent

Styles mirror a SMACSS/BEM-like separation using module-scoped `CSSStyleSheet` objects.

### Sheet modules

- `src/styles/sheets/base-sheet.ts`
- `src/styles/sheets/theme-sheet.ts`
- `src/styles/sheets/utils-sheet.ts`
- `src/styles/sheets/layout-sheet.ts`
- `src/styles/sheets/module-sheet.ts`
- `src/styles/sheets/state-sheet.ts`
- `src/styles/sheets/docs-view-sheet.ts`

### CSS sources

- `src/styles/css/base.css`
- `src/styles/css/theme.css`
- `src/styles/css/utils.css`
- `src/styles/css/layout.css`
- `src/styles/css/module.css`
- `src/styles/css/state.css`
- `src/styles/css/docs-view.css`

### Adoption order

Components adopt sheets from generic to specific.

Recommended order:

1. `base` (reset/foundation)
2. `theme` (tokens/variables)
3. `utils` (low-specificity helpers)
4. `layout` (structural layout)
5. `module` (shared widget styles)
6. `state` (state modifiers)
7. `view-specific` (page/view modules, e.g. docs)

Current examples:

- `oatty-site-app`: `[base, theme, utils, layout, module, state, docs-view]`
- `theme-switcher`: `[base, theme, utils, module]`

### Reuse behavior

Sheet instances are exported once per module and reused across adopters.
This allows consistent updates without redefining stylesheet instances per component.

## Docs Architecture (Phase 1)

Implemented foundation:

- `/docs/quick-start` route
- persistent docs shell layout
  - left docs navigation
  - center content
  - right "On this page" TOC
  - bottom prev/next nav
- reusable summary card pattern ("What you'll learn")

Out of scope for this phase:

- telemetry step-completion hooks
- full Markdown ingestion pipeline
- advanced search UX

## Decision Rationale

Why this was done now:

- Docs surface area will grow quickly.
- Without early layering rules, styles and rendering responsibilities will couple into one large component and force a high-cost refactor.
- A lightweight standard now enables incremental extraction later.

## Evolution Plan

1. Extract docs shell into its own web component.
2. Extract summary card, TOC, and pager into standalone widgets.
3. Move docs page content into dedicated view components.
4. Optionally add a markdown/content pipeline when page volume justifies it.

## Non-Goals

- Introducing a full SPA framework/router.
- Rebuilding the current marketing page architecture immediately.
- Large visual redesign while structural work is in progress.
