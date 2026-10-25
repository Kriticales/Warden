/**
 * Backend simulado do pack aberto para os testes de componente: um pack pronto, o inventário,
 * os detalhes e os ajustes, com respostas que o teste pode trocar.
 */
import type {
  ConfigFileList,
  ConfigOverview,
  Inventory,
  PackRow,
  PlacedConfig,
} from '../../lib/ipc/bindings';
import { mockBackend, type Handler } from '../../test/backend';
import { makePackRow } from '../../test/factories';
import { makeProfilesView } from '../test/profiles/profiles.fixtures';
import {
  DECISIONS,
  makeChoice,
  makeInventory,
  makeMeta,
  makeTestSettingsView,
} from './editor.fixtures';
import { makeGithubStatus } from '../github/github.fixtures';
import { makeXrayView } from './xray/xray.fixtures';

export interface EditorBackendOptions {
  pack?: Partial<PackRow>;
  inventory?: Inventory;
  handlers?: Record<string, Handler>;
}

/** As configs pessoais padrão (`DEFAULT_PERSONAL` da `warden-instance`). */
const PERSONAL = [
  'options.txt',
  'optionsof.txt',
  'optionsshaders.txt',
  'config/xaerominimap.txt',
  'config/xaeroworldmap.txt',
];

/**
 * A lista juntada da seção Configs (`config_overview`, ADR-0067) a partir do `config_tree`
 * simulado do teste (pack e instância), como o Rust junta: o que está no pack é "No pack", o
 * resto da instância é "Só no teste" e o `options.txt` (e parentes) é pessoal. Os arquivos
 * nos dois lados contam como iguais (os testes que precisam da diferença trocam o handler).
 */
export async function overviewFromTree(
  tree: Handler | undefined,
  packId: unknown,
  personalPaths: readonly string[] = PERSONAL,
): Promise<ConfigOverview> {
  const empty: ConfigFileList = { available: false, files: [], truncated: false };
  const pack = tree ? ((await tree({ packId, origin: 'pack' })) as ConfigFileList) : empty;
  const test = tree ? ((await tree({ packId, origin: 'instance' })) as ConfigFileList) : empty;
  const personal = (path: string) => personalPaths.includes(path.toLowerCase());
  const inPack = new Map(pack.files.map((file) => [file.path, file]));
  const inTest = new Map(test.files.map((file) => [file.path, file]));
  const paths = [...new Set([...inPack.keys(), ...inTest.keys()])].sort((a, b) =>
    a.toLowerCase().localeCompare(b.toLowerCase()),
  );
  const files: PlacedConfig[] = paths.map((path) => {
    const file = inTest.get(path) ?? inPack.get(path);
    return {
      path,
      place: personal(path) ? 'personal' : inPack.has(path) ? 'pack' : 'test',
      inPack: inPack.has(path),
      inTest: inTest.has(path),
      size: file?.size ?? 0,
      modifiedAtMs: file?.modifiedAtMs ?? null,
      binary: file?.binary ?? false,
      differs: false,
      differences: 0,
      byKey: true,
      bringable: !personal(path) && !path.startsWith('saves/'),
    };
  });
  return {
    testAvailable: test.available,
    files,
    truncated: pack.truncated || test.truncated,
    personalInPack: paths.filter((path) => personal(path) && inPack.has(path)),
  };
}

export function editorBackend({ pack, inventory, handlers = {} }: EditorBackendOptions = {}) {
  const row = makePackRow({ name: 'Vale Sereno', unsavedFiles: 0, ...pack });
  const backend = mockBackend({
    packs_list: () => [row],
    pack_get: () => row,
    pack_hygiene_scan: () => [],
    // Publicação (V-03): pack ainda não publicado.
    github_status: () => makeGithubStatus(),
    inventory_list: () => inventory ?? makeInventory(),
    pack_meta_get: () =>
      makeMeta({
        name: row.name,
        minecraft: row.minecraft,
        loader: row.loader,
        loaderVersion: row.loaderVersion,
      }),
    // Pack sem relações: o bloco "Dependências" dos detalhes (D-07) não encontra nada.
    graph_dependents: () => ({
      targets: [],
      dependsOn: [],
      embedded: [],
      usedBy: [],
      affected: [],
    }),
    graph_why_in_pack: (args) => ({
      item: { path: String(args.path), name: String(args.path) },
      why: { kind: 'noDependents' },
      cycle: [],
    }),
    pack_test_settings_get: () => makeTestSettingsView(),
    // Perfis do teste (L-08): só o Padrão.
    test_profiles_get: () => makeProfilesView({ minecraft: row.minecraft }),
    item_option_get: () => null,
    // Raio-x de mixins (D-10): o mod não altera o jogo por mixins.
    xray_mod: () => makeXrayView(),
    instance_optional_choices_get: () => [],
    java_choice: () => makeChoice(DECISIONS.forge1201),
    // Configs (ADR-0067): a lista juntada sai do `config_tree` do teste; sem ele, vazia.
    config_overview: (args) => overviewFromTree(handlers.config_tree, args.packId),
    config_compare: (args) => ({
      path: String(args.path),
      byKey: true,
      keys: [],
      lines: 0,
      packText: null,
      testText: null,
    }),
    java_runtimes_list: () => ({
      runtimes: [],
      packsToDownload: [],
      unresolved: [],
      brokenCount: 0,
    }),
    ...handlers,
  });
  return { backend, row, url: `/packs/${row.id}` };
}
