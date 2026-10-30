# Oatty site

Static HTML and CSS site for https://oatty.io. The homepage and all documentation routes are complete in the HTTP response. A small clipboard script enhances the setup prompt on the homepage and API access guide; the full prompt remains selectable without JavaScript.

## Commands

- `npm ci` installs build dependencies.
- `npm run build` writes the site to `dist/`.
- `npm test` checks the static validator and clipboard behavior without dependencies.
- `npm run check:static` verifies page structure, the clipboard-only script policy, prompt parity, and local links.
- `npm run dev` builds and serves the site at http://127.0.0.1:4173.
- `npm run preview` serves the existing build.

Serve the built site on a different address or port with `OATTY_SITE_HOST` and `OATTY_SITE_PORT`. For example, from `oatty.io/`, run `OATTY_SITE_HOST=0.0.0.0 OATTY_SITE_PORT=8080 npm run dev` to view it from another device on your network. The server must serve `dist/`; serving `index.html` from the source directory leaves `/site.css`, images, and documentation routes unresolved.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the content sources and route migration.
