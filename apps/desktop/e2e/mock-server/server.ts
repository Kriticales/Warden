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
  '.xml': 'application/xml; charset=utf-8',
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

/**
 * Respostas reais gravadas pelas crates, servidas no mesmo caminho da fonte oficial (o app
 * aponta cada fonte para este servidor com `WARDEN_API_BASE_*`). Catálogo de versões (P1-05):
 * `crates/warden-catalog/tests/fixtures/http/FIXTURES.md` diz de onde veio cada arquivo.
 */
const CATALOG = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  '..',
  'crates',
  'warden-catalog',
  'tests',
  'fixtures',
  'http',
);
const MODRINTH = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  '..',
  'crates',
  'warden-modrinth',
  'tests',
  'fixtures',
  'http',
);
const RECORDED: Record<string, string> = {
  'mc/game/version_manifest_v2.json': join(CATALOG, '2026-10-05-mojang-version_manifest_v2.json'),
  'v2/versions/game': join(CATALOG, '2026-10-05-fabric-versions-game.json'),
  'v2/versions/loader': join(CATALOG, '2026-10-05-fabric-versions-loader.json'),
  'net/minecraftforge/forge/maven-metadata.xml': join(
    CATALOG,
    '2026-10-05-forge-maven-metadata.xml',
  ),
  'net/minecraftforge/forge/promotions_slim.json': join(
    CATALOG,
    '2026-10-05-forge-promotions_slim.json',
  ),
  'api/maven/versions/releases/net/neoforged/neoforge': join(
    CATALOG,
    '2026-10-05-neoforge-versions-neoforge.json',
  ),
  'api/maven/versions/releases/net/neoforged/forge': join(
    CATALOG,
    '2026-10-05-neoforge-versions-forge.json',
  ),
  // Modrinth (P1-08: inventário e detalhes), com o app em `WARDEN_API_BASE_MODRINTH=<url>/modrinth/v2/`.
  // `crates/warden-modrinth/tests/fixtures/http/FIXTURES.md` diz de onde veio cada arquivo.
  'modrinth/v2/projects': join(MODRINTH, '2026-10-04-projects-3.json'),
  'modrinth/v2/versions': join(MODRINTH, '2026-10-04-versions-2.json'),
  'modrinth/v2/project/AANobbMI': join(MODRINTH, '2026-10-04-project-sodium.json'),
  'modrinth/v2/version/SMxNOGZ6': join(MODRINTH, '2026-10-04-version-SMxNOGZ6.json'),
  // Adicionar (P1-09): toda busca devolve a busca gravada por "sodium" (Fabric 1.21.1).
  'modrinth/v2/search': join(MODRINTH, '2026-10-04-search-sodium-fabric-1.21.1.json'),
  'modrinth/v2/project/AANobbMI/version': join(
    MODRINTH,
    '2026-10-04-project-sodium-versions-fabric-1.21.1.json',
  ),
};

/** O arquivo de fixture para um caminho de URL, ou `null` se sair da pasta. */
export function fixturePath(urlPath: string): string | null {
  const clean = decodeURIComponent(urlPath.split('?')[0] ?? '').replace(/^\/+/, '');
  const recorded = RECORDED[clean];
  if (recorded) return recorded;
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
