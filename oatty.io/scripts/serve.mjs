/** Serve the built site with directory-index routing for local preview. */
import {createServer} from 'node:http';
import {readFile, stat} from 'node:fs/promises';
import {extname, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = resolve(fileURLToPath(new URL('../dist/', import.meta.url)));
const host = process.env.OATTY_SITE_HOST ?? '127.0.0.1';
const port = Number(process.env.OATTY_SITE_PORT ?? '4173');
if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('OATTY_SITE_PORT must be an integer from 1 to 65535');
}
const contentTypes = {'.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.svg': 'image/svg+xml', '.png': 'image/png', '.webp': 'image/webp', '.xml': 'application/xml', '.txt': 'text/plain; charset=utf-8', '.md': 'text/markdown; charset=utf-8'};

createServer(async (request, response) => {
    try {
        const pathname = decodeURIComponent(new URL(request.url ?? '/', 'http://localhost').pathname);
        let filePath = resolve(root, `.${pathname}`);
        if (filePath !== root && !filePath.startsWith(root + sep)) throw new Error('Invalid path');
        if ((await stat(filePath)).isDirectory()) filePath = resolve(filePath, 'index.html');
        const content = await readFile(filePath);
        response.writeHead(200, {'content-type': contentTypes[extname(filePath)] ?? 'application/octet-stream'});
        response.end(content);
    } catch {
        response.writeHead(404, {'content-type': 'text/html; charset=utf-8'});
        response.end(await readFile(resolve(root, '404.html')));
    }
}).listen(port, host, () => console.log(`Oatty preview: http://${host}:${port}`));
