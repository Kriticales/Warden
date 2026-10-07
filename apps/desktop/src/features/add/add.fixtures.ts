/**
 * Dados de teste da página Adicionar: resultados, páginas, pré-visualização, versões e planos,
 * com os campos dos bindings (nomes e IDs reais do Modrinth).
 */
import type {
  AddPlan,
  PlanNode,
  ProjectPreview,
  ProjectVersions,
  SearchPage,
  SearchResult,
  SourceRef,
} from '../../lib/ipc/bindings';

export function ref(projectId: string, overrides: Partial<SourceRef> = {}): SourceRef {
  return {
    source: 'modrinth',
    projectId,
    slug: projectId.toLowerCase(),
    downloads: 1000,
    ...overrides,
  };
}

export function makeResult(overrides: Partial<SearchResult> = {}): SearchResult {
  const sources = overrides.sources ?? [ref('AANobbMI', { slug: 'sodium' })];
  return {
    key: `${sources[0]?.source ?? 'modrinth'}:${sources[0]?.projectId ?? ''}`,
    title: 'Sodium',
    author: 'jellysquid3',
    summary: 'Motor de renderização mais rápido.',
    iconUrl: null,
    downloads: 236_787_190,
    updated: new Date().toISOString(),
    inPack: false,
    compatible: true,
    manualDownload: false,
    ...overrides,
    sources,
  };
}

export const SODIUM = makeResult();
export const MODMENU = makeResult({
  title: 'Mod Menu',
  author: 'Prospector',
  summary: 'Lista os mods no menu.',
  sources: [ref('mOgUt4GM', { slug: 'modmenu' })],
});
export const APPLESKIN = makeResult({
  title: 'AppleSkin',
  author: 'squeek502',
  summary: 'Mostra a fome e a saturação.',
  sources: [ref('EsAfCjCV', { slug: 'appleskin' })],
});
export const LITHIUM = makeResult({
  title: 'Lithium',
  author: 'jellysquid3',
  summary: 'Otimiza a lógica do jogo.',
  inPack: true,
  sources: [ref('gvQqBUqZ', { slug: 'lithium' })],
});
export const OLD_MOD = makeResult({
  title: 'Mod Antigo',
  author: 'alguém',
  compatible: false,
  sources: [ref('OLD12345', { slug: 'mod-antigo' })],
});

export function makePage(items: SearchResult[], overrides: Partial<SearchPage> = {}): SearchPage {
  return {
    items,
    sources: ['modrinth'],
    total: items.length,
    next: null,
    warnings: [],
    ...overrides,
  };
}

export function makePreview(overrides: Partial<ProjectPreview> = {}): ProjectPreview {
  return {
    source: 'modrinth',
    projectId: 'AANobbMI',
    slug: 'sodium',
    title: 'Sodium',
    summary: 'Motor de renderização mais rápido.',
    body: '# Sodium\n\nDeixa o jogo **mais leve**.',
    bodyFormat: 'markdown',
    iconUrl: null,
    downloads: 236_787_190,
    updated: new Date().toISOString(),
    license: 'Polyform Shield 1.0.0',
    side: 'client',
    links: {
      page: 'https://modrinth.com/mod/sodium',
      issues: 'https://github.com/CaffeineMC/sodium/issues',
      source: 'https://github.com/CaffeineMC/sodium',
      wiki: null,
      discord: null,
    },
    ...overrides,
  };
}

export function makeVersions(overrides: Partial<ProjectVersions> = {}): ProjectVersions {
  return {
    versions: [
      {
        id: 'BETA0001',
        number: 'mc1.21.1-0.8.14-beta.1',
        channel: 'beta',
        published: '2026-10-01T00:00:00Z',
        fileName: 'sodium-beta.jar',
        sha1: null,
      },
      {
        id: 'SMxNOGZ6',
        number: 'mc1.21.1-0.8.13',
        channel: 'release',
        published: '2026-09-20T00:00:00Z',
        fileName: 'sodium.jar',
        sha1: null,
      },
    ],
    defaultId: 'SMxNOGZ6',
    ...overrides,
  };
}

export function makeNode(overrides: Partial<PlanNode> = {}): PlanNode {
  const projectId = overrides.projectId ?? 'mOgUt4GM';
  return {
    key: `modrinth:${projectId}`,
    role: 'chosen',
    source: 'modrinth',
    projectId,
    slug: 'modmenu',
    title: 'Mod Menu',
    iconUrl: null,
    kind: 'mod',
    versionId: 'V-MODMENU',
    versionNumber: '11.0.5',
    channel: 'release',
    outsideChannel: false,
    compatible: true,
    fileName: 'modmenu.jar',
    side: 'client',
    sideNote: null,
    requiredBy: [],
    optionalFor: [],
    ...overrides,
  };
}

/** Mod Menu e AppleSkin escolhidos; os dois exigem a Fabric API (CA-T09-05). */
export function makePlan(overrides: Partial<AddPlan> = {}): AddPlan {
  return {
    target: { minecraft: '1.21.1', loader: 'fabric' },
    nodes: [
      makeNode(),
      makeNode({
        projectId: 'EsAfCjCV',
        slug: 'appleskin',
        title: 'AppleSkin',
        versionId: 'V-APPLESKIN',
        versionNumber: '3.0.6+mc1.21',
      }),
      makeNode({
        projectId: 'P7dR8mSH',
        slug: 'fabric-api',
        title: 'Fabric API',
        role: 'required',
        versionId: 'V-FABRIC-API',
        versionNumber: '0.116.17+1.21.1',
        requiredBy: ['modrinth:mOgUt4GM', 'modrinth:EsAfCjCV'],
      }),
      makeNode({
        projectId: 'eXts2L7r',
        slug: 'placeholder-api',
        title: 'Text Placeholder API',
        role: 'required',
        versionId: 'V-PLACEHOLDER',
        versionNumber: '2.4.2+1.21',
        requiredBy: ['modrinth:mOgUt4GM'],
      }),
    ],
    installed: [],
    missing: [],
    conflicts: [],
    duplicates: [],
    packItemCount: 4,
    ...overrides,
  };
}
