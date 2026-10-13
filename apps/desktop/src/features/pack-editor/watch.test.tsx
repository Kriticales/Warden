/**
 * Mudança externa (A-05): toda consulta que mostra dados do pack é relida, sem trocar de seção,
 * quando o vigia manda `pack-changed` com `external: true`, mesmo as que ficam fora das áreas que
 * o vigia manda (mod editado por fora só traz Inventário; config, só Configs).
 */
import { QueryClientProvider, useQueries, type QueryKey } from '@tanstack/react-query';
import { act, render, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { createQueryClient } from '../../app/App';
import { Toaster } from '../../components/ui/toast';
import type { PackArea } from '../../lib/ipc/bindings';
import { usePackChangedInvalidation } from '../../lib/ipc/events';
import { mockBackend } from '../../test/backend';
import { addKeys, type SearchFilters } from '../add/common/api';
import { discoverKeys } from '../add/discover/api';
import { previewKeys } from '../add/preview/api';
import { configsKeys } from '../configs/api';
import { exportKeys } from '../export/api';
import { formatKeys } from '../export/formats/api';
import { prismKeys } from '../export/prism/api';
import { packsKeys } from '../packs/api';
import { manualDownloadKeys } from '../test/blocked/api';
import { testKeys } from '../test/api';
import { updatesKeys } from '../updates/api';
import { versioningKeys } from '../versioning/api';
import { editorKeys } from './api';
import { graphKeys } from './details/dependencies/api';
import { optionKeys } from './item-options/api';
import { usePackWatch } from './watch';

const PACK = 'p1';
const ITEM = 'mods/sodium.pw.toml';

/** Cada consulta do app que mostra dados do pack aberto (as chaves de `*Keys`). */
const QUERIES: readonly (readonly [string, QueryKey])[] = [
  ['cabeçalho (linha do pack)', packsKeys.row(PACK)],
  ['Meus packs', packsKeys.list],
  ['higiene', packsKeys.hygiene(PACK)],
  ['Mods', editorKeys.inventory(PACK)],
  ['detalhes do item', editorKeys.details(PACK, ITEM)],
  ['plano de remoção', editorKeys.removalPlan(PACK, [ITEM])],
  ['informações do pack', editorKeys.meta(PACK)],
  ['ajustes do teste', editorKeys.testSettings(PACK)],
  ['opções do item', optionKeys.option(PACK, ITEM)],
  ['opcionais do pack', optionKeys.choices(PACK)],
  ['grafo: quem depende', graphKeys.dependents(PACK, ITEM)],
  ['grafo: por que está no pack', graphKeys.why(PACK, ITEM)],
  ['Configs: árvore', configsKeys.tree(PACK, 'pack')],
  ['Configs: arquivo', configsKeys.file(PACK, 'instance', 'config/sodium.json')],
  ['Histórico: alterações não salvas', versioningKeys.history(PACK)],
  ['Histórico: o que mudou desde uma versão', versioningKeys.changes(PACK, '1.4.2')],
  ['Salvar versão: prévia', versioningKeys.preview(PACK)],
  ['Salvar versão: número', versioningKeys.validate(PACK, '1.5.0')],
  ['pontos de segurança', versioningKeys.safety(PACK)],
  ['Exportar: prévia', exportKeys.preview(PACK)],
  ['Exportar: Prism', prismKeys.analysis(PACK)],
  ['Exportar: formato', formatKeys.analysis(PACK, 'mrpack')],
  ['atualizações: relatório', updatesKeys.report(PACK)],
  ['atualizações: revisão', updatesKeys.plan(PACK, [ITEM])],
  ['adicionar: busca', addKeys.search(PACK, {} as SearchFilters)],
  ['adicionar: projeto', addKeys.details(PACK, 'modrinth', 'sodium')],
  ['adicionar: versões', addKeys.versions(PACK, 'modrinth', 'sodium')],
  ['adicionar: plano', addKeys.plan(PACK, [])],
  ['descoberta', discoverKeys.home(PACK, 'mod')],
  ['prévia: galeria', previewKeys.gallery(PACK, 'modrinth', 'sodium')],
  ['prévia: notas', previewKeys.notes(PACK, 'modrinth', 'sodium', 'v1')],
  ['prévia: dependências', previewKeys.dependencies(PACK, 'modrinth', 'v1')],
  ['Problemas: downloads manuais', manualDownloadKeys.list(PACK)],
  ['testes gravados', testKeys.sessions(PACK)],
  ['sessão de teste', testKeys.session(PACK, 's1')],
  ['mundos do teste', testKeys.worlds(PACK)],
];

/** A tela do pack: o vigia ligado e uma consulta ativa por chave, contando as leituras. */
function renderPackScreen() {
  const backend = mockBackend();
  const client = createQueryClient();
  const reads = new Map<string, number>(QUERIES.map(([name]) => [name, 0]));
  function Screen() {
    usePackChangedInvalidation(client);
    usePackWatch(PACK);
    useQueries({
      queries: QUERIES.map(([name, queryKey]) => ({
        queryKey,
        queryFn: () => {
          reads.set(name, (reads.get(name) ?? 0) + 1);
          return reads.get(name) ?? 0;
        },
        staleTime: Infinity,
      })),
    });
    return null;
  }
  render(
    <QueryClientProvider client={client}>
      <Screen />
      <Toaster />
    </QueryClientProvider>,
  );
  return { backend, reads };
}

/** O que ainda não foi relido desde `before`. */
function notReread(reads: Map<string, number>, before: Map<string, number>): string[] {
  return QUERIES.map(([name]) => name).filter(
    (name) => (reads.get(name) ?? 0) <= (before.get(name) ?? 0),
  );
}

describe('mudança externa no pack aberto (A-05)', () => {
  it.each<[string, PackArea[]]>([
    ['um mod editado por fora (o vigia manda só Inventário)', ['inventory']],
    ['uma config editada por fora (o vigia manda só Configs)', ['configs']],
  ])('%s relê todas as consultas do pack e Meus packs', async (_case, areas) => {
    const { backend, reads } = renderPackScreen();
    await waitFor(() => {
      expect(notReread(reads, new Map())).toEqual([]);
    });
    const before = new Map(reads);

    // O ouvinte do evento liga sem esperar; repete até ele estar ouvindo.
    await waitFor(async () => {
      await act(() => backend.emit('pack-changed', { packId: PACK, areas, external: true }));
      expect(notReread(reads, before)).toEqual([]);
    });
  });

  it('escrita do próprio Warden relê só as áreas que ela mudou (o resto é das mutations)', async () => {
    const { backend, reads } = renderPackScreen();
    await waitFor(() => {
      expect(notReread(reads, new Map())).toEqual([]);
    });
    const before = new Map(reads);

    await waitFor(async () => {
      await act(() =>
        backend.emit('pack-changed', { packId: PACK, areas: ['configs'], external: false }),
      );
      expect(reads.get('Configs: árvore')).toBeGreaterThan(before.get('Configs: árvore') ?? 0);
    });
    expect(notReread(reads, before)).toContain('Histórico: alterações não salvas');
    expect(notReread(reads, before)).toContain('Exportar: prévia');
  });
});
