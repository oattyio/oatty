/** Assemble the authored HTML pages and static assets for GitHub Pages. */
import {cp, mkdir, readFile, readdir, rm, writeFile} from 'node:fs/promises';
import {dirname, join} from 'node:path';

const SITE_ORIGIN = 'https://oatty.io';
const OUTPUT_DIRECTORY = 'dist';
const DOCS_SOURCE_DIRECTORY = 'static-docs';

/** Copy each documentation page and return its public route for the sitemap. */
async function copyDocumentationPages() {
    const files = await readdir(DOCS_SOURCE_DIRECTORY, {recursive: true});
    const pages = files.filter((file) => file.endsWith('/index.html'));
    for (const page of pages) {
        const output = join(OUTPUT_DIRECTORY, 'docs', page);
        await mkdir(dirname(output), {recursive: true});
        await cp(join(DOCS_SOURCE_DIRECTORY, page), output);
    }
    return pages.map((page) => `/docs/${page.slice(0, -'/index.html'.length)}/`);
}

/** Keep the historical /docs/ entry point pointed at the quick-start guide. */
async function writeDocumentationAlias() {
    const quickStart = await readFile(join(OUTPUT_DIRECTORY, 'docs/quick-start/index.html'), 'utf8');
    const alias = quickStart.replace('</head>', '  <meta name="robots" content="noindex, follow">\n</head>');
    await writeFile(join(OUTPUT_DIRECTORY, 'docs/index.html'), alias);
}

/** Write discovery files from the same authored route set used for the build. */
async function writeDiscoveryFiles(routes) {
    await writeFile(join(OUTPUT_DIRECTORY, 'robots.txt'), `User-agent: *\nAllow: /\nSitemap: ${SITE_ORIGIN}/sitemap.xml\n`);
    const entries = ['/', ...routes].map((route) => `<url><loc>${SITE_ORIGIN}${route}</loc></url>`).join('');
    await writeFile(join(OUTPUT_DIRECTORY, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${entries}</urlset>`);
}

/** Rebuild the complete site without a browser-side application or page renderer. */
async function buildSite() {
    await rm(OUTPUT_DIRECTORY, {recursive: true, force: true});
    await mkdir(OUTPUT_DIRECTORY, {recursive: true});
    await cp('public', OUTPUT_DIRECTORY, {recursive: true});
    await cp('index.html', join(OUTPUT_DIRECTORY, 'index.html'));
    const routes = await copyDocumentationPages();
    await writeDocumentationAlias();
    await cp('static-pages/404.html', join(OUTPUT_DIRECTORY, '404.html'));
    await writeDiscoveryFiles(routes);
    await mkdir(join(OUTPUT_DIRECTORY, 'runbooks'), {recursive: true});
    await cp('docs/Sentry_Datadog_PagerDuty_Integration_Playbook.md', join(OUTPUT_DIRECTORY, 'runbooks/sentry-datadog-pagerduty.md'));
}

buildSite().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
