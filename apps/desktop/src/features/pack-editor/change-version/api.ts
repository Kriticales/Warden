/**
 * Dados de "Trocar de versão" e da versão do loader (P1-15).
 *
 * Regra do disco como verdade: sem atualização otimista. Trocar a versão de um item muda o
 * inventário (lista, detalhes, grafo e relatório de atualizações ficam na área `inventory`), a
 * linha do pack (alterações não salvas) e o Histórico. Trocar a versão do loader muda também as
 * informações do pack (`meta`) e o cabeçalho, que lê a linha do pack.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type Loader, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { loaderVersionsQuery, packsKeys } from '../../packs/api';

export const changeVersionKeys = {
  /** As versões compatíveis de um item (somem junto quando o inventário muda). */
  options: (packId: PackId, path: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'versionOptions', path] as const,
  /** O que muda com a versão escolhida. */
  plan: (packId: PackId, path: string, versionId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'versionPlan', path, versionId] as const,
  /** A verificação dos mods contra outra versão do loader. */
  loaderCheck: (packId: PackId, version: string) =>
    [...queryKeys.packArea(packId, 'meta'), 'loaderCheck', version] as const,
};

/** Tudo o que mostra os itens, a linha do pack e o Histórico. */
export function itemsChanged(packId: PackId) {
  return [
    queryKeys.packArea(packId, 'inventory'),
    queryKeys.packArea(packId, 'history'),
    packsKeys.row(packId),
    packsKeys.list,
  ] as const;
}

export function useVersionOptions(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(changeVersionKeys.options(packId, path), () =>
      commands.itemVersionOptions(packId, path),
    ),
    enabled,
    staleTime: 0,
  });
}

export function useVersionPlan(packId: PackId, path: string, versionId: string | null) {
  return useQuery({
    ...commandQuery(changeVersionKeys.plan(packId, path, versionId ?? ''), () =>
      commands.itemVersionPlan(packId, path, versionId ?? ''),
    ),
    enabled: versionId !== null,
    staleTime: 0,
  });
}

export function useChangeVersion(packId: PackId) {
  return useCommandMutation(
    ({
      path,
      expectedCurrentId,
      versionId,
    }: {
      path: string;
      expectedCurrentId: string | null;
      versionId: string;
    }) => commands.itemVersionChange(packId, path, expectedCurrentId, versionId),
    { invalidates: itemsChanged(packId) },
  );
}

export function useLoaderVersions(loader: Loader, minecraft: string, enabled: boolean) {
  return useQuery({ ...loaderVersionsQuery(loader, minecraft), enabled });
}

export function useLoaderCheck(packId: PackId, version: string | null) {
  return useQuery({
    ...commandQuery(changeVersionKeys.loaderCheck(packId, version ?? ''), () =>
      commands.loaderVersionCheck(packId, version ?? ''),
    ),
    enabled: version !== null,
    staleTime: 0,
  });
}

export function useSetLoaderVersion(packId: PackId) {
  return useCommandMutation(
    ({ expectedFrom, version }: { expectedFrom: string; version: string }) =>
      commands.loaderVersionSet(packId, expectedFrom, version),
    { invalidates: [...itemsChanged(packId), queryKeys.packArea(packId, 'meta')] },
  );
}
