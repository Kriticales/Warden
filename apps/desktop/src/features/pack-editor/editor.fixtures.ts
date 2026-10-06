/**
 * Dados de teste do pack aberto. Os itens "reais" vêm dos metafiles que o packwiz gerou de
 * verdade (`crates/warden-packwiz/tests/fixtures/packwiz-output/`): Sodium (Modrinth, versão
 * `SMxNOGZ6`) e Applied Energistics 2 (CurseForge, arquivo 7148487).
 */
import type {
  Inventory,
  InventoryItem,
  ItemDetails,
  JavaChoice,
  JavaDecision,
  PackMeta,
  TestSettingsView,
} from '../../lib/ipc/bindings';

let counter = 0;

export function makeItem(overrides: Partial<InventoryItem> = {}): InventoryItem {
  counter += 1;
  const name = overrides.name ?? `Mod ${String(counter)}`;
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  return {
    key: `path:mods/${slug}.pw.toml`,
    path: `mods/${slug}.pw.toml`,
    kind: 'mod',
    state: 'ok',
    name,
    fileName: `${slug}-1.0.0.jar`,
    version: '1.0.0',
    source: 'modrinth',
    side: 'both',
    sideEditable: true,
    pinned: false,
    optional: false,
    projectId: null,
    sourceVersionId: null,
    summary: null,
    iconUrl: null,
    error: null,
    ...overrides,
  };
}

/** Sodium como o packwiz grava (Modrinth). */
export const SODIUM = makeItem({
  key: 'modrinth:AANobbMI',
  path: 'mods/sodium.pw.toml',
  name: 'Sodium',
  fileName: 'sodium-fabric-0.8.13+mc1.21.1.jar',
  version: '0.8.13+mc1.21.1',
  source: 'modrinth',
  side: 'client',
  pinned: true,
  projectId: 'AANobbMI',
  sourceVersionId: 'SMxNOGZ6',
  summary: 'The fastest and most compatible rendering optimization mod for Minecraft.',
});

/** Applied Energistics 2 como o packwiz grava (CurseForge). */
export const AE2 = makeItem({
  key: 'curseforge:223794',
  path: 'mods/applied-energistics-2.pw.toml',
  name: 'Applied Energistics 2',
  fileName: 'appliedenergistics2-forge-15.4.10.jar',
  version: 'appliedenergistics2-forge-15.4.10',
  source: 'curseforge',
  side: 'both',
  projectId: '223794',
  sourceVersionId: '7148487',
});

export const SHADER = makeItem({
  key: 'modrinth:HVnmMxH1',
  path: 'shaderpacks/complementary-reimagined.pw.toml',
  kind: 'shader',
  name: 'Complementary Shaders - Reimagined',
  fileName: 'ComplementaryReimagined_r5.5.1.zip',
  version: 'r5.5.1',
  side: 'client',
});

export const RESOURCE_PACK = makeItem({
  key: 'modrinth:50dA9Sha',
  path: 'resourcepacks/fresh-animations.pw.toml',
  kind: 'resourcePack',
  name: 'Fresh Animations',
  fileName: 'FreshAnimations_v1.9.4.zip',
  version: 'v1.9.4',
  side: 'client',
});

export const INVALID = makeItem({
  key: 'path:mods/sodium-extra.pw.toml',
  path: 'mods/sodium-extra.pw.toml',
  state: 'invalid',
  name: 'sodium-extra.pw.toml',
  fileName: null,
  version: null,
  side: 'unknown',
  sideEditable: false,
  error: 'mods/sodium-extra.pw.toml: TOML inválido na linha 3: falta fechar as aspas',
});

export const OUTSIDE = makeItem({
  key: 'path:mods/kotlinforforge-4.11.0-all.jar',
  path: 'mods/kotlinforforge-4.11.0-all.jar',
  state: 'outsideIndex',
  name: 'kotlinforforge-4.11.0-all.jar',
  fileName: 'kotlinforforge-4.11.0-all.jar',
  version: 'kotlinforforge-4.11.0-all',
  source: 'local',
  sideEditable: false,
});

export function makeInventory(
  items: InventoryItem[] = [SODIUM, AE2, RESOURCE_PACK, SHADER],
  indexError: string | null = null,
): Inventory {
  return { items, indexError };
}

export function makeDetails(
  item: InventoryItem,
  overrides: Partial<ItemDetails> = {},
): ItemDetails {
  return {
    item,
    source: 'live',
    title: item.name,
    authors: [],
    pageUrl: null,
    iconUrl: null,
    summary: item.summary,
    description: null,
    descriptionFormat: 'markdown',
    version: null,
    file: {
      fileName: item.fileName ?? '',
      hashFormat: 'sha512',
      hash: '8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e',
      url: null,
    },
    changelog: null,
    ...overrides,
  };
}

export function makeMeta(overrides: Partial<PackMeta> = {}): PackMeta {
  return {
    name: 'Vale Sereno',
    author: 'Jogador',
    description: '',
    version: '1.4.2',
    minecraft: '1.20.1',
    loader: 'forge',
    loaderVersion: '47.3.0',
    ...overrides,
  };
}

export function makeTestSettingsView(overrides: Partial<TestSettingsView> = {}): TestSettingsView {
  return {
    settings: { memory: { mode: 'auto' }, java: null, jvmArgs: '' },
    instanceExists: false,
    hasWorlds: false,
    ...overrides,
  };
}

/**
 * Decisões da política da L-01 (`crates/warden-java/data/java-compat.toml`), como o
 * `java_choice` devolve: o teste de Rust `pack_meta::tests::politica_do_java_*` confere as mesmas
 * três com a tabela real.
 */
export const DECISIONS = {
  /** Forge 1.20.1 → Java 17, porque um Java mais novo não foi provado com a faixa. */
  forge1201: {
    requirement: { major: 17, maxUpdate: null },
    reason: 'NEWEST_PROVEN_FOR_RANGE',
    ruleId: '1.17-a-1.20.4',
    rangeLabel: '1.17 a 1.20.4',
    explanation:
      'Do Minecraft 1.17 ao 1.20.4 o jogo pede o Java 17 (o 1.17.x pedia o 16, que o Adoptium não publica e que o 17 substitui); um Java mais novo ainda não foi provado nessa faixa.',
    newestMajor: 25,
    versionJsonMajor: null,
  },
  /** Forge 1.7.10 → Java 8, porque o Forge antigo trava do Java 9 em diante. */
  forge1710: {
    requirement: { major: 8, maxUpdate: null },
    reason: 'FORGE_LEGACY_JAVA8',
    ruleId: 'forge-antigo-so-java-8',
    rangeLabel: 'até o 1.12.2',
    explanation:
      'O Forge do Minecraft 1.7.10 ao 1.12.2 só abre no Java 8: a partir do Java 9 ele trava ao iniciar (o launchwrapper converte o carregador de classes do sistema para URLClassLoader, o que falha com ClassCastException).',
    newestMajor: 25,
    versionJsonMajor: null,
  },
  /** A versão mais nova do Minecraft → o Java mais novo, sem explicação extra. */
  newest: {
    requirement: { major: 25, maxUpdate: null },
    reason: 'NEWEST_AVAILABLE',
    ruleId: '26.x',
    rangeLabel: '26.x',
    explanation: 'As versões 26.x do Minecraft pedem o Java 25, o mais novo que o Warden baixa.',
    newestMajor: 25,
    versionJsonMajor: null,
  },
} satisfies Record<string, JavaDecision>;

export function makeChoice(decision: JavaDecision): JavaChoice {
  return {
    major: decision.requirement.major,
    maxUpdate: decision.requirement.maxUpdate,
    reason: decision.reason,
    runtime: null,
    automatic: decision,
    userChoiceUnavailable: false,
  };
}
