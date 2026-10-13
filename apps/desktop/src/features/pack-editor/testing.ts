/**
 * Backend simulado do pack aberto para os testes de componente: um pack pronto, o inventário,
 * os detalhes e os ajustes, com respostas que o teste pode trocar.
 */
import type { Inventory, PackRow } from '../../lib/ipc/bindings';
import { mockBackend, type Handler } from '../../test/backend';
import { makePackRow } from '../../test/factories';
import {
  DECISIONS,
  makeChoice,
  makeInventory,
  makeMeta,
  makeTestSettingsView,
} from './editor.fixtures';
import { makeGithubStatus } from '../github/github.fixtures';

export interface EditorBackendOptions {
  pack?: Partial<PackRow>;
  inventory?: Inventory;
  handlers?: Record<string, Handler>;
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
    graph_dependents: () => ({ targets: [], dependsOn: [], usedBy: [], affected: [] }),
    graph_why_in_pack: (args) => ({
      item: { path: String(args.path), name: String(args.path) },
      why: { kind: 'noDependents' },
      cycle: [],
    }),
    pack_test_settings_get: () => makeTestSettingsView(),
    item_option_get: () => null,
    instance_optional_choices_get: () => [],
    java_choice: () => makeChoice(DECISIONS.forge1201),
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
