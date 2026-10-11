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
 */

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

/** SHA-1 do arquivo estável do Sodium para Fabric 1.21.1 no Modrinth (`SMxNOGZ6`). */
const SODIUM_SHA1 = '003c114c85ca88ef3362e018deb6aca0c682d6a1';

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

const FILES = [
  file(CF_SODIUM, 'sodium-fabric-0.6.0+mc1.21.1.jar', SODIUM_SHA1),
  file(CF_ONLY, 'mod-so-da-curseforge-1.0.jar', '1'.repeat(40), [[CF_LIB, 3]]),
  file(CF_LIB, 'biblioteca-1.0.jar', '2'.repeat(40)),
  file(CF_BLOCKED, 'mod-bloqueado-1.0.jar', '3'.repeat(40)),
];

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
 * Responde a uma requisição à CurseForge simulada. `path` é o caminho depois de `/curseforge/`
 * (por exemplo `mods/search`). Devolve `null` para o que não existe.
 */
export function curseforgeReply(
  method: string,
  path: string,
  key: string | undefined,
  body: string,
): Reply | null {
  if (key === CHAVE_COM_ERRO) return { status: 500, body: { error: 'simulado' } };
  if (!key || key === CHAVE_RECUSADA) return { status: 403, body: {} };
  const route = path.replace(/^\/+|\/+$/g, '');
  if (method === 'GET' && route === 'mods/search') return { status: 200, body: paged(PROJECTS) };
  if (method === 'POST' && route === 'mods') {
    const wanted = ids(body, 'modIds');
    return { status: 200, body: { data: PROJECTS.filter((p) => wanted.includes(p.id)) } };
  }
  if (method === 'POST' && route === 'mods/files') {
    const wanted = ids(body, 'fileIds');
    return { status: 200, body: { data: FILES.filter((f) => wanted.includes(f.id)) } };
  }
  const files = /^mods\/(\d+)\/files$/.exec(route);
  if (method === 'GET' && files) {
    return { status: 200, body: paged(FILES.filter((f) => f.modId === Number(files[1]))) };
  }
  const description = /^mods\/(\d+)\/description$/.exec(route);
  if (method === 'GET' && description) {
    return { status: 200, body: { data: '<p>Descrição simulada da CurseForge.</p>' } };
  }
  const one = /^mods\/(\d+)$/.exec(route);
  if (method === 'GET' && one) {
    const found = PROJECTS.find((p) => p.id === Number(one[1]));
    return found ? { status: 200, body: { data: found } } : { status: 404, body: {} };
  }
  return null;
}
