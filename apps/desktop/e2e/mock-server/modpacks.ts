/**
 * Modpacks simulados dos E2E (P1-17), nas duas fontes:
 *
 * - **Modrinth** ("Pacote Leve", `MPleve01`): a busca com `project_type:modpack` devolve só ele;
 *   a versão do modpack (Fabric 1.21.1) lista como `embedded` mods cujas respostas reais já estão
 *   gravadas no servidor de fixtures (Sodium e Lithium, que o pack de teste já tem; spark e Crash
 *   Assistant, compatíveis; Rubidium, sem versão para Fabric 1.21.1) e um jar fora das lojas.
 * - **CurseForge** ("Vale CurseForge", `910001`): montado aqui, como o resto da CurseForge
 *   simulada (os termos dela proíbem guardar respostas). O endereço de download aponta para um
 *   "edge" que responde 404 a qualquer pedido com `Range` e redireciona sem ele para um "media"
 *   que aceita `Range` (R7 §9): o app precisa resolver o redirecionamento antes de ler o
 *   `manifest.json` por partes. O manifesto cita os mods da CurseForge simulada (`curseforge.ts`).
 *
 * O projeto do modpack do Modrinth (para `/projects?ids=`) está em
 * `fixtures/modpacks/modrinth-projects.json`, junto dos gravados em `BY_IDS` (`server.ts`).
 *
 * Os textos `MANIFEST_MARKER` e `API_MARKER` só existem no manifesto e na resposta da API: o
 * teste procura os dois na pasta de dados depois da sessão (CA-T08-16).
 */
import { crc32 } from 'node:zlib';

import { CF_ONLY, CF_SODIUM } from './curseforge';

export const MODRINTH_MODPACK = 'MPleve01';
const MODRINTH_VERSION = 'MPv00001';
export const CF_MODPACK = 910_001;
const CF_MODPACK_FILE = 9_100_010;
export const MANIFEST_MARKER = 'Manifesto-do-E2E-P117';
export const API_MARKER = 'Resposta-da-API-do-E2E-P117';

/** Caminhos do arquivo do modpack da CurseForge neste servidor. */
const EDGE_PATH = `/cf-edge/files/9100/010/vale-curseforge-1.0.zip`;
const MEDIA_PATH = `/cf-media/files/9100/010/vale-curseforge-1.0.zip`;

export interface ModpackReply {
  status: number;
  headers: Record<string, string>;
  body: Buffer | string;
}

function json(body: unknown): ModpackReply {
  return {
    status: 200,
    headers: { 'content-type': 'application/json; charset=utf-8' },
    body: JSON.stringify(body),
  };
}

/** Um zip simples (entradas `stored`), suficiente para o crate `zip` do app. */
function zip(entries: [string, Buffer][]): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of entries) {
    const nameBytes = Buffer.from(name, 'utf8');
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

/** O zip do modpack da CurseForge: 200 configs (~400 KB) antes do manifesto e um jar solto. */
const CF_ZIP = zip([
  ...Array.from({ length: 200 }, (_, i): [string, Buffer] => [
    `overrides/config/config-${String(i)}.toml`,
    Buffer.alloc(2_000, 'x'),
  ]),
  ['overrides/mods/feito-a-mao-1.0.jar', Buffer.from('PK')],
  [
    'manifest.json',
    Buffer.from(
      JSON.stringify({
        minecraft: { version: '1.21.1', modLoaders: [{ id: 'fabric-0.16.5', primary: true }] },
        manifestType: 'minecraftModpack',
        manifestVersion: 1,
        name: MANIFEST_MARKER,
        version: '1.0.0',
        author: 'Autora',
        files: [
          { projectID: CF_SODIUM, fileID: CF_SODIUM * 10, required: true },
          { projectID: CF_ONLY, fileID: CF_ONLY * 10, required: true },
        ],
        overrides: 'overrides',
      }),
      'utf8',
    ),
  ],
]);

/** O arquivo com `Range` (206) ou inteiro (200), como uma CDN. */
function serveRange(data: Buffer, range: string | undefined): ModpackReply {
  const spec = range?.startsWith('bytes=') ? range.slice('bytes='.length) : null;
  if (spec === null) {
    return {
      status: 200,
      headers: { 'content-type': 'application/zip', 'accept-ranges': 'bytes' },
      body: data,
    };
  }
  const [startText = '', endText = ''] = spec.split('-');
  const start = startText === '' ? Math.max(0, data.length - Number(endText)) : Number(startText);
  const end =
    startText === '' || endText === ''
      ? data.length - 1
      : Math.min(Number(endText), data.length - 1);
  return {
    status: 206,
    headers: {
      'content-type': 'application/zip',
      'content-range': `bytes ${String(start)}-${String(end)}/${String(data.length)}`,
    },
    body: data.subarray(start, end + 1),
  };
}

const MODRINTH_HIT = {
  project_id: MODRINTH_MODPACK,
  project_type: 'modpack',
  slug: 'pacote-leve',
  author: 'Autora',
  title: 'Pacote Leve',
  description: 'Um modpack leve para Fabric 1.21.1.',
  categories: ['fabric', 'optimization'],
  display_categories: ['optimization'],
  versions: ['1.21.1'],
  downloads: 12_345,
  follows: 10,
  icon_url: '',
  date_created: '2026-01-01T00:00:00Z',
  date_modified: '2026-10-01T00:00:00Z',
  latest_version: MODRINTH_VERSION,
  license: 'MIT',
  client_side: 'required',
  server_side: 'optional',
  gallery: [],
};

const MODRINTH_VERSION_BODY = [
  {
    id: MODRINTH_VERSION,
    project_id: MODRINTH_MODPACK,
    author_id: 'autora',
    name: 'Pacote Leve 1.0.0',
    version_number: '1.0.0',
    version_type: 'release',
    game_versions: ['1.21.1'],
    loaders: ['fabric'],
    featured: true,
    status: 'listed',
    date_published: '2026-10-01T00:00:00Z',
    downloads: 100,
    files: [
      {
        hashes: { sha1: '0'.repeat(40), sha512: '0'.repeat(128) },
        url: 'https://cdn.modrinth.com/data/MPleve01/versions/MPv00001/pacote-leve.mrpack',
        filename: 'pacote-leve.mrpack',
        primary: true,
        size: 1000,
      },
    ],
    dependencies: [
      { project_id: 'AANobbMI', version_id: 'SMxNOGZ6', dependency_type: 'embedded' },
      { project_id: 'gvQqBUqZ', version_id: null, dependency_type: 'embedded' },
      { project_id: 'l6YH9Als', version_id: 'cALUj9l1', dependency_type: 'embedded' },
      { project_id: 'ix1qq8Ux', version_id: 'AYajytbk', dependency_type: 'embedded' },
      { project_id: '4ZqxOvjD', version_id: null, dependency_type: 'embedded' },
      { file_name: 'ajustes-do-autor-1.0.jar', dependency_type: 'embedded' },
    ],
  },
];

function cfModpack() {
  return {
    id: CF_MODPACK,
    gameId: 432,
    name: 'Vale CurseForge',
    slug: 'vale-curseforge',
    links: { websiteUrl: 'https://www.curseforge.com/minecraft/modpacks/vale-curseforge' },
    summary: API_MARKER,
    status: 4,
    downloadCount: 640_000,
    classId: 4471,
    authors: [{ id: 2, name: 'Autora' }],
    logo: null,
    latestFilesIndexes: [
      {
        gameVersion: '1.21.1',
        fileId: CF_MODPACK_FILE,
        filename: 'vale-curseforge-1.0.zip',
        releaseType: 1,
        modLoader: 4,
      },
    ],
    mainFileId: CF_MODPACK_FILE,
    dateCreated: '2026-01-01T00:00:00Z',
    dateModified: '2026-10-01T00:00:00Z',
    allowModDistribution: true,
    isAvailable: true,
  };
}

/**
 * A resposta de modpack para o pedido, ou `null` se o pedido não for de modpack (o servidor
 * segue com as outras respostas). `path` é o caminho com a consulta.
 */
export function modpackReply(
  method: string,
  path: string,
  apiKey: string | undefined,
  range: string | undefined,
  origin: string,
): ModpackReply | null {
  const url = new URL(path, 'http://fixtures.invalid/');
  const route = url.pathname;
  if (method === 'GET' && route === '/modrinth/v2/search') {
    const facets = url.searchParams.get('facets') ?? '';
    if (!facets.includes('project_type:modpack')) return null;
    return json({ hits: [MODRINTH_HIT], offset: 0, limit: 20, total_hits: 1 });
  }
  if (method === 'GET' && route === `/modrinth/v2/project/${MODRINTH_MODPACK}/version`) {
    return json(MODRINTH_VERSION_BODY);
  }
  if (route.startsWith('/curseforge/') && !apiKey) return null;
  if (method === 'GET' && route === '/curseforge/mods/search') {
    if (url.searchParams.get('classId') !== '4471') return null;
    const data = [cfModpack()];
    return json({
      data,
      pagination: { index: 0, pageSize: 1, resultCount: 1, totalCount: 1 },
    });
  }
  if (method === 'GET' && route === `/curseforge/mods/${String(CF_MODPACK)}`) {
    return json({ data: cfModpack() });
  }
  if (
    method === 'GET' &&
    route === `/curseforge/mods/${String(CF_MODPACK)}/files/${String(CF_MODPACK_FILE)}/download-url`
  ) {
    return json({ data: `${origin}${EDGE_PATH}` });
  }
  if (route === EDGE_PATH) {
    if (range !== undefined) return { status: 404, headers: {}, body: '' };
    return { status: 302, headers: { location: `${origin}${MEDIA_PATH}` }, body: '' };
  }
  if (route === MEDIA_PATH) {
    if (method === 'HEAD') return { status: 200, headers: { 'accept-ranges': 'bytes' }, body: '' };
    return serveRange(CF_ZIP, range);
  }
  return null;
}
