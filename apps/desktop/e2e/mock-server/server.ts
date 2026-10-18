/**
 * Servidor local de fixtures dos E2E (QUALITY §4.1 e §13.5): APIs externas simuladas, sem
 * rede. Escuta só em 127.0.0.1, numa porta livre, e serve os arquivos de `fixtures/` pelo
 * caminho (`GET /health` → `fixtures/health.json`). As tarefas que falam com APIs
 * (Modrinth, CurseForge, GitHub) acrescentam as suas respostas gravadas em `fixtures/` e
 * apontam o app para `WARDEN_E2E_MOCK_URL`.
 */
import { readFile } from 'node:fs/promises';
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { extname, join, normalize, sep } from 'node:path';

import { CF_IMAGE_PREFIX, curseforgeImage, curseforgeReply } from './curseforge';
import { githubState, resetGithub, serveGithub } from './github';
import { DIRECT_PATH, directJar, sha512, sodiumJar } from './jars';
import { modpackReply } from './modpacks';
import { servePlayer } from './player';
// Itens inventados do pack grande dos E2E de desempenho (A-04).
import { perfModrinthItems } from '../perf/modrinth';

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
  /** Caminhos pedidos, na ordem. Os testes (outro processo) leem pelo `GET /__requests`. */
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
const PROJECT = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  '..',
  'crates',
  'warden-project',
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
  // `/projects?ids=` e `/versions?ids=` são respondidos por `byIds` (abaixo).
  'modrinth/v2/project/AANobbMI': join(MODRINTH, '2026-10-04-project-sodium.json'),
  // Link do Modrinth colado no campo (P1-11): o projeto pelo slug.
  'modrinth/v2/project/sodium': join(MODRINTH, '2026-10-04-project-sodium.json'),
  'modrinth/v2/version/SMxNOGZ6': join(MODRINTH, '2026-10-04-version-SMxNOGZ6.json'),
  // Adicionar (P1-09): toda busca devolve a busca gravada por "sodium" (Fabric 1.21.1).
  'modrinth/v2/search': join(MODRINTH, '2026-10-04-search-sodium-fabric-1.21.1.json'),
  'modrinth/v2/project/AANobbMI/version': join(
    MODRINTH,
    '2026-10-04-project-sodium-versions-fabric-1.21.1.json',
  ),
  // Mods iniciais (P1-18): spark, Crash Assistant e Fabric API para Fabric 1.21.1
  // (`crates/warden-project/tests/fixtures/http/FIXTURES.md`). Para outro loader ou versão, o
  // app descarta as versões que não servem, e o item aparece "Sem versão para este pack".
  'modrinth/v2/project/l6YH9Als/version': join(
    PROJECT,
    '2026-10-07-versions-spark-fabric-1.21.1.json',
  ),
  'modrinth/v2/project/ix1qq8Ux/version': join(
    PROJECT,
    '2026-10-07-versions-crash-assistant-fabric-1.21.1.json',
  ),
  'modrinth/v2/project/P7dR8mSH/version': join(
    PROJECT,
    '2026-10-07-versions-fabric-api-fabric-1.21.1.json',
  ),
};

/**
 * `GET /projects?ids=[…]` e `GET /versions?ids=[…]`: como a API, devolve só os pedidos (por ID
 * ou slug), tirados da união das respostas gravadas.
 */
const BY_IDS: Record<string, string[]> = {
  'modrinth/v2/projects': [
    join(MODRINTH, '2026-10-04-projects-3.json'),
    join(PROJECT, '2026-10-07-projects-initial.json'),
    join(PROJECT, '2026-10-07-projects-add.json'),
    // Navegar por modpacks (P1-17): o projeto do modpack simulado do Modrinth (`modpacks.ts`).
    join(FIXTURES, 'modpacks', 'modrinth-projects.json'),
  ],
  'modrinth/v2/versions': [
    join(MODRINTH, '2026-10-04-versions-2.json'),
    join(PROJECT, '2026-10-07-versions-spark-fabric-1.21.1.json'),
    join(PROJECT, '2026-10-07-versions-crash-assistant-fabric-1.21.1.json'),
    join(PROJECT, '2026-10-07-versions-fabric-api-fabric-1.21.1.json'),
  ],
};

/** A resposta de `BY_IDS` para o caminho, ou `null` se o caminho não for um deles. */
async function byIds(urlPath: string): Promise<string | null> {
  const url = new URL(urlPath, 'http://fixtures');
  const files = BY_IDS[decodeURIComponent(url.pathname).replace(/^\/+/, '')];
  if (!files) return null;
  const wanted = JSON.parse(url.searchParams.get('ids') ?? '[]') as string[];
  const seen = new Set<string>();
  const found: unknown[] = [];
  for (const file of files) {
    const items = JSON.parse(await readFile(file, 'utf8')) as { id: string; slug?: string }[];
    for (const item of items) {
      if (seen.has(item.id)) continue;
      if (wanted.includes(item.id) || (item.slug !== undefined && wanted.includes(item.slug))) {
        seen.add(item.id);
        found.push(item);
      }
    }
  }
  found.push(...perfModrinthItems(decodeURIComponent(url.pathname).replace(/^\/+/, ''), wanted));
  return JSON.stringify(found);
}

/** O arquivo de fixture para um caminho de URL, ou `null` se sair da pasta. */
export function fixturePath(urlPath: string): string | null {
  const clean = decodeURIComponent(urlPath.split('?')[0] ?? '').replace(/^\/+/, '');
  const recorded = RECORDED[clean];
  if (recorded) return recorded;
  const withExtension = extname(clean) ? clean : `${clean}.json`;
  const resolved = normalize(join(FIXTURES, withExtension));
  return resolved.startsWith(FIXTURES + sep) ? resolved : null;
}

/** A CurseForge simulada (P1-10): respostas montadas em `curseforge.ts`, nada em disco. */
function serveCurseforge(request: IncomingMessage, response: ServerResponse, path: string): void {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    const route = path.slice('/curseforge/'.length);
    const key = request.headers['x-api-key'];
    const reply = curseforgeReply(
      request.method ?? 'GET',
      route,
      Array.isArray(key) ? key[0] : key,
      Buffer.concat(chunks).toString('utf8'),
      `http://${request.headers.host ?? '127.0.0.1'}`,
    );
    response
      .writeHead(reply?.status ?? 404, { 'content-type': 'application/json; charset=utf-8' })
      .end(JSON.stringify(reply?.body ?? {}));
  });
}

/**
 * `POST /version_files` do Modrinth: a resposta de `fixtures/modrinth/v2/version_files.json`
 * (atualizações, P1-12) e, quando o pedido traz o sha512 do jar sintético do Sodium
 * (`jars.ts`; Adicionar arquivo local, P1-11), também a versão gravada do Sodium.
 */
function serveVersionFiles(request: IncomingMessage, response: ServerResponse): void {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    void (async () => {
      const body = JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}') as {
        hashes?: string[];
      };
      const known = sha512(sodiumJar);
      const found = JSON.parse(
        await readFile(join(FIXTURES, 'modrinth', 'v2', 'version_files.json'), 'utf8'),
      ) as Record<string, unknown>;
      if (body.hashes?.includes(known)) {
        found[known] = JSON.parse(
          await readFile(join(MODRINTH, '2026-10-04-version-SMxNOGZ6.json'), 'utf8'),
        ) as unknown;
      }
      response.writeHead(200, { 'content-type': TYPES['.json'] }).end(JSON.stringify(found));
    })().catch(() => {
      response.writeHead(500).end();
    });
  });
}

/** Serve o arquivo de fixture do caminho (404 se não existir; 400 se sair da pasta). */
function serveFile(path: string, response: ServerResponse): void {
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
}

export async function startMockServer(): Promise<MockServer> {
  const requests: string[] = [];
  let port = 0;
  const base = () => `http://127.0.0.1:${String(port)}`;
  const server: Server = createServer((request, response) => {
    const path = request.url ?? '/';
    // Os testes rodam em outro processo: veem o registro (e o zeram) por aqui.
    if (path === '/__requests') {
      response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(requests));
      return;
    }
    // O GitHub simulado (V-03): repositórios e Releases criados pelo app.
    if (path === '/__github') {
      response
        .writeHead(200, { 'content-type': 'application/json' })
        .end(JSON.stringify(githubState()));
      return;
    }
    if (path === '/__github/reset' && request.method === 'POST') {
      resetGithub();
      response.writeHead(204).end();
      return;
    }
    if (path === '/__reset') {
      requests.length = 0;
      response.writeHead(204).end();
      return;
    }
    requests.push(`${request.method ?? 'GET'} ${path}`);
    // Navegar por modpacks (P1-17): busca, versão, API e arquivo dos modpacks simulados.
    const apiKey = request.headers['x-api-key'];
    const range = request.headers.range;
    const modpack = modpackReply(
      request.method ?? 'GET',
      path,
      Array.isArray(apiKey) ? apiKey[0] : apiKey,
      range,
      base(),
    );
    if (modpack) {
      response.writeHead(modpack.status, modpack.headers).end(modpack.body);
      return;
    }
    if (path.startsWith(CF_IMAGE_PREFIX)) {
      const image = curseforgeImage(path);
      if (image) response.writeHead(200, { 'content-type': 'image/png' }).end(image);
      else response.writeHead(404).end();
      return;
    }
    if (path.startsWith('/curseforge/')) {
      serveCurseforge(request, response, path);
      return;
    }
    if (path.startsWith('/github/')) {
      serveGithub(request, response, path, base());
      return;
    }
    // "Testar como o jogador recebe" (L-07): os jars do instalador e os mods do pack de teste.
    if (servePlayer(path, response)) {
      return;
    }
    if (request.method === 'POST' && path.split('?')[0] === '/modrinth/v2/version_files') {
      serveVersionFiles(request, response);
      return;
    }
    // O link direto `https://files.example.com/…` (P1-11), com o app em `WARDEN_E2E_DIRECT_VIA`.
    if (path === DIRECT_PATH) {
      response.writeHead(200, { 'content-type': TYPES['.jar'] }).end(directJar);
      return;
    }
    void byIds(path)
      .then((body) => {
        if (body === null) {
          serveFile(path, response);
        } else {
          response.writeHead(200, { 'content-type': TYPES['.json'] }).end(body);
        }
      })
      .catch(() => {
        response.writeHead(500).end();
      });
  });
  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('servidor de fixtures sem porta');
  }
  port = address.port;
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
