/**
 * Servidor local de fixtures dos E2E (QUALITY §4.1 e §13.5): APIs externas simuladas, sem
 * rede. Escuta só em 127.0.0.1, numa porta livre, e serve os arquivos de `fixtures/` pelo
 * caminho (`GET /health` → `fixtures/health.json`). As tarefas que falam com APIs
 * (Modrinth, CurseForge, GitHub) acrescentam as suas respostas gravadas em `fixtures/` e
 * apontam o app para `WARDEN_E2E_MOCK_URL`.
 */
import { readFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import { extname, join, normalize, sep } from 'node:path';

const FIXTURES = join(import.meta.dirname, 'fixtures');

const TYPES: Record<string, string> = {
  '.json': 'application/json; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.jar': 'application/java-archive',
  '.zip': 'application/zip',
};

export interface MockServer {
  /** `http://127.0.0.1:<porta>`. */
  url: string;
  /** Caminhos pedidos, na ordem. */
  requests: string[];
  close: () => Promise<void>;
}

/** O arquivo de fixture para um caminho de URL, ou `null` se sair da pasta. */
export function fixturePath(urlPath: string): string | null {
  const clean = decodeURIComponent(urlPath.split('?')[0] ?? '').replace(/^\/+/, '');
  const withExtension = extname(clean) ? clean : `${clean}.json`;
  const resolved = normalize(join(FIXTURES, withExtension));
  return resolved.startsWith(FIXTURES + sep) ? resolved : null;
}

export async function startMockServer(): Promise<MockServer> {
  const requests: string[] = [];
  const server: Server = createServer((request, response) => {
    const path = request.url ?? '/';
    requests.push(`${request.method ?? 'GET'} ${path}`);
    const file = fixturePath(path);
    if (!file) {
      response.writeHead(400).end();
      return;
    }
    readFile(file)
      .then((body) => {
        response
          .writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' })
          .end(body);
      })
      .catch(() => {
        response.writeHead(404).end();
      });
  });
  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('servidor de fixtures sem porta');
  }
  return {
    url: `http://127.0.0.1:${String(address.port)}`,
    requests,
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) reject(error);
          else resolve();
        });
      }),
  };
}
