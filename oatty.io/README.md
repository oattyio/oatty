# Oatty site

Static HTML and CSS site for https://oatty.io. The homepage and all documentation routes are complete in the HTTP response, with no client-side JavaScript.

## Commands

- `npm ci` installs build dependencies.
- `npm run build` writes the site to `dist/`.
- `npm run check:static` verifies page structure, script-free output, and local links.
- `npm run dev` builds and serves the site at http://127.0.0.1:4173.
- `npm run preview` serves the existing build.

Serve the built site on a different address or port with `OATTY_SITE_HOST` and `OATTY_SITE_PORT`. For example, from `oatty.io/`, run `OATTY_SITE_HOST=0.0.0.0 OATTY_SITE_PORT=8080 npm run dev` to view it from another device on your network. The server must serve `dist/`; serving `index.html` from the source directory leaves `/site.css`, images, and documentation routes unresolved.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the content sources and route migration.
