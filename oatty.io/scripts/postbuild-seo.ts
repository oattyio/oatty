import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {join} from 'node:path';

import {docsPages} from '../src/docs/pages';
import type {DocsPage, DocsSection} from '../src/docs/types';

const SITE_ORIGIN = 'https://oatty.io';
const DIST_DIRECTORY = 'dist';

type DocsRouteEntry = {
    docsPage: DocsPage;
    canonicalPath?: string;
    includeInSitemap: boolean;
    robotsDirective?: string;
};

/**
 * Generate route-aware SEO and prerender artifacts after Vite completes the production build.
 */
async function runPostBuildSearchEngineOptimizationTasks(): Promise<void> {
    const builtIndexHtmlPath = join(DIST_DIRECTORY, 'index.html');
    const builtIndexHtml = await readFile(builtIndexHtmlPath, 'utf8');
    const docsRouteMap = buildDocsRouteMap();

    await generatePrerenderedDocsShells(builtIndexHtml, docsRouteMap);
    await generateRobotsFile();
    await generateSitemapFile(docsRouteMap);
    await writeFile(join(DIST_DIRECTORY, '404.html'), builtIndexHtml, 'utf8');
}

/**
 * Create route map used for sitemap generation and docs-route prerendering.
 */
function buildDocsRouteMap(): Map<string, DocsRouteEntry> {
    const routeMap = new Map<string, DocsRouteEntry>();
    for (const page of docsPages) {
        routeMap.set(page.path, {docsPage: page, includeInSitemap: true});
    }

    // Provide `/docs` alias as a convenience entry path that mirrors quick start, but avoid duplicate indexing.
    const quickStartPage = docsPages.find((page) => page.path === '/docs/quick-start');
    if (quickStartPage) {
        routeMap.set('/docs', {
            docsPage: quickStartPage,
            canonicalPath: '/docs/quick-start',
            includeInSitemap: false,
            robotsDirective: 'noindex, follow',
        });
    }

    return routeMap;
}

/**
 * Generate static route shells with prerendered docs content and route-specific metadata.
 */
async function generatePrerenderedDocsShells(baseHtml: string, docsRouteMap: Map<string, DocsRouteEntry>): Promise<void> {
    for (const [routePath, entry] of docsRouteMap) {
        const docsPage = entry.docsPage;
        const canonicalPath = entry.canonicalPath ?? routePath;

        const routeDirectory = join(DIST_DIRECTORY, routePath.slice(1));
        await mkdir(routeDirectory, {recursive: true});

        const prerenderedHtml = applyRouteMetadata(
            injectFallbackMarkup(baseHtml, renderDocsFallbackMarkup(docsPage)),
            canonicalPath,
            `${docsPage.title} | Oatty Docs`,
            docsPage.summary,
            entry.robotsDirective,
        );

        await writeFile(join(routeDirectory, 'index.html'), prerenderedHtml, 'utf8');
    }
}

/**
 * Replace title/meta/canonical tags with route-aware values for static prerendered shells.
 */
function applyRouteMetadata(html: string, canonicalPath: string, pageTitle: string, pageDescription: string, robotsDirective?: string): string {
    const canonicalUrl = `${SITE_ORIGIN}${canonicalPath}`;

    let updatedHtml = html;
    updatedHtml = replaceTagContent(updatedHtml, /<title>[\s\S]*?<\/title>/, `<title>${escapeHtml(pageTitle)}</title>`);
    updatedHtml = replaceMetaTag(updatedHtml, 'name', 'title', pageTitle);
    updatedHtml = replaceMetaTag(updatedHtml, 'name', 'description', pageDescription);
    updatedHtml = replaceMetaTag(updatedHtml, 'property', 'og:title', pageTitle);
    updatedHtml = replaceMetaTag(updatedHtml, 'property', 'og:description', pageDescription);
    updatedHtml = replaceMetaTag(updatedHtml, 'property', 'og:url', canonicalUrl);
    updatedHtml = replaceMetaTag(updatedHtml, 'name', 'twitter:title', pageTitle);
    updatedHtml = replaceMetaTag(updatedHtml, 'name', 'twitter:description', pageDescription);
    updatedHtml = replaceMetaTag(updatedHtml, 'name', 'twitter:url', canonicalUrl);
    updatedHtml = replaceTagContent(updatedHtml, /<link rel="canonical" href="[^"]*"\s*\/?>/, `<link rel="canonical" href="${canonicalUrl}"/>`);
    if (robotsDirective) {
        updatedHtml = replaceMetaTag(updatedHtml, 'name', 'robots', robotsDirective);
    }

    return updatedHtml;
}

/**
 * Replace the fallback content inside the custom element with prerendered docs HTML.
 */
function injectFallbackMarkup(baseHtml: string, fallbackMarkup: string): string {
    return baseHtml.replace(/<oatty-site-app>[\s\S]*?<\/oatty-site-app>/, `<oatty-site-app>${fallbackMarkup}</oatty-site-app>`);
}

/**
 * Render docs page content as crawlable light-DOM fallback HTML.
 */
function renderDocsFallbackMarkup(page: DocsPage): string {
    const renderedSections = page.sections.map(renderDocsSection).join('');
    const renderedLearningPoints = page.learnBullets.map((bullet) => `<li>${escapeHtml(bullet)}</li>`).join('');

    return `
    <main id="main-content">
        <article>
            <header>
                <h1>${escapeHtml(page.title)}</h1>
                <p>${escapeHtml(page.summary)}</p>
                <p><strong>Estimated time:</strong> ${escapeHtml(page.estimatedTime)}</p>
            </header>
            <section>
                <h2>What You Will Learn</h2>
                <ul>
                    ${renderedLearningPoints}
                </ul>
            </section>
            ${renderedSections}
        </article>
    </main>
    `;
}

/**
 * Render one docs section with heading, paragraphs, optional code sample, and callouts.
 */
function renderDocsSection(section: DocsSection): string {
    const headingLevel = section.headingLevel ?? 2;
    const renderedParagraphs = section.paragraphs.map((paragraph) => `<p>${escapeHtml(paragraph)}</p>`).join('');
    const renderedCode = section.codeSample
        ? `<pre><code>${escapeHtml(section.codeSample)}</code></pre>`
        : '';
    const renderedCallouts = section.callouts?.map((callout) => {
        if (callout.type === 'screenshot') {
            return `
            <figure>
                <img src="${escapeHtml(callout.imageSrc)}" alt="${escapeHtml(callout.imageAlt)}"/>
                <figcaption>${escapeHtml(callout.content)}</figcaption>
            </figure>
            `;
        }

        const calloutLabel = callout.label ? `${escapeHtml(callout.label)}: ` : '';
        return `<p><strong>${escapeHtml(callout.type)}</strong> ${calloutLabel}${escapeHtml(callout.content)}</p>`;
    }).join('') ?? '';

    return `
    <section id="${escapeHtml(section.id)}">
        <h${headingLevel}>${escapeHtml(section.title)}</h${headingLevel}>
        ${renderedParagraphs}
        ${renderedCode}
        ${renderedCallouts}
    </section>
    `;
}

/**
 * Generate robots file for crawl directives and sitemap discovery.
 */
async function generateRobotsFile(): Promise<void> {
    const robotsFileContent = ['User-agent: *', 'Allow: /', `Sitemap: ${SITE_ORIGIN}/sitemap.xml`, ''].join('\n');
    await writeFile(join(DIST_DIRECTORY, 'robots.txt'), robotsFileContent, 'utf8');
}

/**
 * Generate sitemap from home route plus docs routes.
 */
async function generateSitemapFile(docsRouteMap: Map<string, DocsRouteEntry>): Promise<void> {
    const isoDate = new Date().toISOString();
    const docsRoutes = Array.from(docsRouteMap.entries())
        .filter(([, entry]) => entry.includeInSitemap)
        .map(([routePath]) => routePath);
    const allRoutes = ['/', ...docsRoutes];
    const uniqueRoutes = Array.from(new Set(allRoutes));

    const sitemapEntries = uniqueRoutes.map((routePath) => {
        const location = routePath === '/' ? SITE_ORIGIN : `${SITE_ORIGIN}${routePath}`;
        const priority = routePath === '/' ? '1.0' : '0.8';
        return `  <url>\n    <loc>${location}</loc>\n    <lastmod>${isoDate}</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>${priority}</priority>\n  </url>`;
    }).join('\n');

    const sitemapFileContent = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
        sitemapEntries,
        '</urlset>',
        '',
    ].join('\n');

    await writeFile(join(DIST_DIRECTORY, 'sitemap.xml'), sitemapFileContent, 'utf8');
}

/**
 * Replace a full tag using a regular expression.
 */
function replaceTagContent(inputHtml: string, pattern: RegExp, replacement: string): string {
    return inputHtml.replace(pattern, replacement);
}

/**
 * Replace a meta tag content value by `name` or `property` selector.
 */
function replaceMetaTag(inputHtml: string, attribute: 'name' | 'property', selectorValue: string, content: string): string {
    const pattern = new RegExp(`<meta\\s+${attribute}="${escapeRegExp(selectorValue)}"\\s+content="[^"]*"\\s*\\/?>(?![\\s\\S]*<meta\\s+${attribute}="${escapeRegExp(selectorValue)}")`);
    const replacement = `<meta ${attribute}="${selectorValue}" content="${escapeHtml(content)}"/>`;
    return inputHtml.replace(pattern, replacement);
}

/**
 * Escape HTML-significant characters for safe inline string rendering.
 */
function escapeHtml(value: string): string {
    return value
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
}

/**
 * Escape user text for regular-expression construction.
 */
function escapeRegExp(value: string): string {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

runPostBuildSearchEngineOptimizationTasks().catch((error: unknown) => {
    console.error('postbuild SEO generation failed:', error);
    process.exitCode = 1;
});
