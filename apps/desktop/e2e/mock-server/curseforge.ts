/**
 * A CurseForge simulada dos E2E (P1-10). As respostas são montadas aqui, no formato da API
 * (Core API v1, campos reduzidos), em vez de gravadas: os termos da CurseForge proíbem guardar
 * respostas dela no repositório (`crates/warden-curseforge/tests/common/mod.rs` faz o mesmo).
 *
 * O app aponta para este servidor com `WARDEN_API_BASE_CURSEFORGE=<url>/curseforge/`. Quem
 * decide a resposta é a chave que o app manda no cabeçalho `x-api-key`:
 *
 * - `CHAVE_RECUSADA`: 403 em tudo (a CurseForge recusando a chave);
 * - `CHAVE_COM_ERRO`: 500 em tudo (a CurseForge fora do ar);
 * - qualquer outra: respostas normais;
 * - sem chave: 403 (o app não deve mandar nada sem chave; o teste confere pelo registro).
 *
 * As capturas de tela do "Mod Só da CurseForge" (galeria da P1-16) são servidas por este mesmo
 * servidor em `/curseforge-img/` (PNG gerado aqui), e o app as busca pelo `warden-img://`.
 */
import { crc32, deflateSync } from 'node:zlib';

export const CHAVE_RECUSADA = 'chave-recusada-do-e2e';
export const CHAVE_COM_ERRO = 'chave-com-erro-do-e2e';
export const CHAVE_BOA = 'chave-boa-do-e2e';

/** Sodium na CurseForge: mesmo autor e slug do Modrinth (vira um item só, nas duas fontes). */
export const CF_SODIUM = 394_468;
/** Mod que só existe na CurseForge e a biblioteca obrigatória dele. */
export const CF_ONLY = 900_001;
export const CF_LIB = 900_002;
/** Mod com a distribuição por terceiros bloqueada. */
export const CF_BLOCKED = 900_003;
/**
 * Mod que entra pelo link do projeto (CA-T08-03). Fica fora da busca, para não mudar o que os
 * outros passos veem; o app e o packwiz o acham pelo ID.
 */
export const CF_LINK = 900_004;

/** SHA-1 do arquivo estável do Sodium para Fabric 1.21.1 no Modrinth (`SMxNOGZ6`). */
const SODIUM_SHA1 = '003c114c85ca88ef3362e018deb6aca0c682d6a1';

/** Caminho das imagens simuladas da CurseForge neste servidor. */
export const CF_IMAGE_PREFIX = '/curseforge-img/';

function chunk(type: string, data: Buffer): Buffer {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'latin1'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

/** Um PNG de uma cor só (RGB, 8 bits), do tamanho dado. */
function png(width: number, height: number, rgb: [number, number, number]): Buffer {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array(width).fill(rgb).flat())]);
  const pixels = Buffer.concat(Array<Buffer>(height).fill(row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(pixels)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A imagem simulada do caminho (`/curseforge-img/<nome>.png`), ou `null`. */
export function curseforgeImage(path: string): Buffer | null {
  const name = path.slice(CF_IMAGE_PREFIX.length);
  if (name === 'print-1.png') return png(320, 180, [30, 90, 110]);
  if (name === 'print-1-thumb.png') return png(160, 90, [30, 90, 110]);
  return null;
}

interface Reply {
  status: number;
  body: unknown;
}

function project(id: number, slug: string, name: string, author: string, blocked = false) {
  return {
    id,
    gameId: 432,
    name,
    slug,
    links: { websiteUrl: `https://www.curseforge.com/minecraft/mc-mods/${slug}` },
    summary: `Resumo de ${name}`,
    status: 4,
    downloadCount: 1000,
    classId: 6,
    authors: [{ id: 1, name: author }],
    logo: null,
    latestFilesIndexes: [
      { gameVersion: '1.21.1', fileId: id * 10, filename: 'a.jar', releaseType: 1, modLoader: 4 },
    ],
    dateCreated: '2020-01-01T00:00:00Z',
    dateModified: '2026-10-01T00:00:00Z',
    allowModDistribution: !blocked,
    isAvailable: true,
  };
}

function file(modId: number, name: string, sha1: string, deps: [number, number][] = []) {
  return {
    id: modId * 10,
    gameId: 432,
    modId,
    isAvailable: true,
    displayName: name,
    fileName: name,
    releaseType: 1,
    fileStatus: 4,
    hashes: [{ value: sha1, algo: 1 }],
    fileDate: '2026-09-19T11:47:57.347Z',
    fileLength: 1000,
    downloadCount: 7,
    downloadUrl: `https://edge.forgecdn.net/files/${String(modId * 10)}/${name}`,
    gameVersions: ['Client', '1.21.1', 'Fabric', 'Server'],
    sortableGameVersions: [
      { gameVersionName: '1.21.1', gameVersion: '1.21.1', gameVersionTypeId: 75125 },
      { gameVersionName: 'Fabric', gameVersion: '', gameVersionTypeId: 68441 },
    ],
    dependencies: deps.map(([depId, relationType]) => ({ modId: depId, relationType })),
    isServerPack: false,
    fileFingerprint: modId * 10,
  };
}

const PROJECTS = [
  project(CF_SODIUM, 'sodium', 'Sodium', 'jellysquid3'),
  project(CF_ONLY, 'mod-so-da-curseforge', 'Mod Só da CurseForge', 'Autora'),
  project(CF_LIB, 'biblioteca-curseforge', 'Biblioteca da CurseForge', 'Autora'),
  project(CF_BLOCKED, 'mod-bloqueado', 'Mod Bloqueado', 'Outro', true),
];

/** Os da busca e o do link. */
const ALL_PROJECTS = [...PROJECTS, project(CF_LINK, 'mod-do-link', 'Mod do Link', 'Autora')];

const FILES = [
  file(CF_SODIUM, 'sodium-fabric-0.6.0+mc1.21.1.jar', SODIUM_SHA1),
  file(CF_ONLY, 'mod-so-da-curseforge-1.0.jar', '1'.repeat(40), [[CF_LIB, 3]]),
  file(CF_LIB, 'biblioteca-1.0.jar', '2'.repeat(40)),
  file(CF_BLOCKED, 'mod-bloqueado-1.0.jar', '3'.repeat(40)),
  file(CF_LINK, 'mod-do-link-1.0.jar', '4'.repeat(40)),
];

/**
 * O projeto com as capturas de tela, como a API devolve na busca e no projeto. Só o "Mod Só da
 * CurseForge" tem uma, servida por este servidor (`origin`).
 */
function withScreenshots(found: ReturnType<typeof project>, origin: string) {
  const screenshots =
    found.id === CF_ONLY
      ? [
          {
            id: 1,
            title: 'Base com o mod',
            description: 'Captura simulada',
            thumbnailUrl: `${origin}${CF_IMAGE_PREFIX}print-1-thumb.png`,
            url: `${origin}${CF_IMAGE_PREFIX}print-1.png`,
          },
        ]
      : [];
  return { ...found, screenshots };
}

function paged<T>(items: T[]) {
  return {
    data: items,
    pagination: {
      index: 0,
      pageSize: items.length,
      resultCount: items.length,
      totalCount: items.length,
    },
  };
}

function ids(body: string, field: string): number[] {
  try {
    const parsed = JSON.parse(body) as Record<string, unknown>;
    const list = parsed[field];
    return Array.isArray(list) ? list.filter((id): id is number => typeof id === 'number') : [];
  } catch {
    return [];
  }
}

/**
 * Responde a uma requisição à CurseForge simulada. `path` é o caminho depois de `/curseforge/`,
 * com a consulta (por exemplo `mods/search?slug=jei`). Devolve `null` para o que não existe.
 */
export function curseforgeReply(
  method: string,
  path: string,
  key: string | undefined,
  body: string,
  origin: string,
): Reply | null {
  if (key === CHAVE_COM_ERRO) return { status: 500, body: { error: 'simulado' } };
  if (!key || key === CHAVE_RECUSADA) return { status: 403, body: {} };
  const url = new URL(path, 'http://curseforge.invalid/');
  const route = url.pathname.replace(/^\/+|\/+$/g, '');
  if (method === 'GET' && route === 'mods/search') {
    // Pelo slug (link de projeto), como a API: só o projeto daquele slug, inclusive o do link.
    const slug = url.searchParams.get('slug');
    if (slug !== null)
      return {
        status: 200,
        body: paged(
          ALL_PROJECTS.filter((p) => p.slug === slug).map((p) => withScreenshots(p, origin)),
        ),
      };
    return { status: 200, body: paged(PROJECTS.map((p) => withScreenshots(p, origin))) };
  }
  if (method === 'POST' && route === 'mods') {
    const wanted = ids(body, 'modIds');
    return {
      status: 200,
      body: {
        data: ALL_PROJECTS.filter((p) => wanted.includes(p.id)).map((p) =>
          withScreenshots(p, origin),
        ),
      },
    };
  }
  if (method === 'POST' && route === 'mods/files') {
    const wanted = ids(body, 'fileIds');
    return { status: 200, body: { data: FILES.filter((f) => wanted.includes(f.id)) } };
  }
  const files = /^mods\/(\d+)\/files$/.exec(route);
  if (method === 'GET' && files) {
    return { status: 200, body: paged(FILES.filter((f) => f.modId === Number(files[1]))) };
  }
  // Um arquivo só: é o que o packwiz pede no `curseforge add --addon-id --file-id`.
  const oneFile = /^mods\/(\d+)\/files\/(\d+)$/.exec(route);
  if (method === 'GET' && oneFile) {
    const found = FILES.find((f) => f.modId === Number(oneFile[1]) && f.id === Number(oneFile[2]));
    return found ? { status: 200, body: { data: found } } : { status: 404, body: {} };
  }
  const changelog = /^mods\/(\d+)\/files\/(\d+)\/changelog$/.exec(route);
  if (method === 'GET' && changelog) {
    return { status: 200, body: { data: '<p>Notas simuladas desta versão.</p>' } };
  }
  const description = /^mods\/(\d+)\/description$/.exec(route);
  if (method === 'GET' && description) {
    return { status: 200, body: { data: '<p>Descrição simulada da CurseForge.</p>' } };
  }
  const one = /^mods\/(\d+)$/.exec(route);
  if (method === 'GET' && one) {
    const found = ALL_PROJECTS.find((p) => p.id === Number(one[1]));
    return found
      ? { status: 200, body: { data: withScreenshots(found, origin) } }
      : { status: 404, body: {} };
  }
  return null;
}
