/**
 * Dados do pack aberto (ARCHITECTURE §18): queries e mutations sobre `inventory_*`, `item_*`,
 * `pack_meta_*` e `pack_test_settings_*`.
 *
 * Regra do disco como verdade (SPEC T05): sem atualização otimista; as mutations invalidam as
 * áreas que mudaram, e o inventário e as informações do pack são relidos do disco ao abrir a
 * seção e quando a janela volta ao foco (CA-T05-01: um arquivo mudado por fora aparece sem
 * reiniciar o app).
 */
import { useQuery } from '@tanstack/react-query';

import {
  commands,
  type MetaUpdate,
  type PackId,
  type SideChoice,
  type TestSettings,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { packsKeys } from '../packs/api';

export const editorKeys = {
  /** A lista de Mods. */
  inventory: (packId: PackId) => [...queryKeys.packArea(packId, 'inventory'), 'list'] as const,
  /** Detalhes de um item (dependem do inventário: some junto quando o pack muda). */
  details: (packId: PackId, path: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'details', path] as const,
  /** O plano de remoção (quem depende dos itens). */
  removalPlan: (packId: PackId, paths: readonly string[]) =>
    [...queryKeys.packArea(packId, 'inventory'), 'removalPlan', ...paths] as const,
  /** Nome, autor e descrição do `pack.toml`. */
  meta: (packId: PackId) => [...queryKeys.packArea(packId, 'meta'), 'pack'] as const,
  /** Ajustes do teste (em `packs.json`, não no pack). */
  testSettings: (packId: PackId) => [...queryKeys.pack(packId), 'testSettings'] as const,
};

/** Sempre relido do disco ao abrir a seção e ao voltar para a janela. */
const fromDisk = {
  staleTime: 0,
  refetchOnMount: 'always',
  refetchOnWindowFocus: 'always',
} as const;

export function useInventory(packId: PackId) {
  return useQuery({
    ...commandQuery(editorKeys.inventory(packId), () => commands.inventoryList(packId)),
    ...fromDisk,
  });
}

export function useItemDetails(packId: PackId, path: string | null) {
  return useQuery({
    ...commandQuery(editorKeys.details(packId, path ?? ''), () =>
      commands.itemDetails(packId, path ?? ''),
    ),
    enabled: path !== null,
    // A CurseForge é consultada na hora e nada fica em disco (SPEC T07); na memória, 1 minuto.
    staleTime: 60_000,
  });
}

export function useRemovalPlan(packId: PackId, paths: readonly string[] | null) {
  return useQuery({
    ...commandQuery(editorKeys.removalPlan(packId, paths ?? []), () =>
      commands.itemsRemovePlan(packId, [...(paths ?? [])]),
    ),
    enabled: paths !== null && paths.length > 0,
    staleTime: 0,
    gcTime: 0,
  });
}

export function usePackMeta(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(editorKeys.meta(packId), () => commands.packMetaGet(packId)),
    ...fromDisk,
    enabled,
  });
}

export function useTestSettings(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(editorKeys.testSettings(packId), () => commands.packTestSettingsGet(packId)),
    staleTime: 0,
    enabled,
  });
}

/** O que muda o inventário também muda a linha do pack (alterações não salvas). */
function inventoryChanged(packId: PackId) {
  return {
    invalidates: [queryKeys.packArea(packId, 'inventory'), packsKeys.row(packId), packsKeys.list],
  } as const;
}

export function useSetSide(packId: PackId) {
  return useCommandMutation(
    ({ paths, side }: { paths: string[]; side: SideChoice }) =>
      commands.itemsSetSide(packId, paths, side),
    inventoryChanged(packId),
  );
}

export function useRemoveItems(packId: PackId) {
  return useCommandMutation(
    (paths: string[]) => commands.itemsRemove(packId, paths),
    inventoryChanged(packId),
  );
}

export function useIncludeOutside(packId: PackId) {
  return useCommandMutation(
    () => commands.inventoryIncludeOutside(packId),
    inventoryChanged(packId),
  );
}

export function useOpenItemFile(packId: PackId) {
  return useCommandMutation((path: string) => commands.itemOpenFile(packId, path));
}

export function useUpdateMeta(packId: PackId) {
  return useCommandMutation((update: MetaUpdate) => commands.packUpdateMeta(packId, update), {
    invalidates: [queryKeys.packArea(packId, 'meta'), packsKeys.row(packId), packsKeys.list],
  });
}

export function useSaveTestSettings(packId: PackId) {
  return useCommandMutation(
    (settings: TestSettings) => commands.packTestSettingsSet(packId, settings),
    { invalidates: [editorKeys.testSettings(packId)] },
  );
}

export function useRecreateInstance(packId: PackId) {
  return useCommandMutation(
    (keepWorlds: boolean) => commands.instanceRecreate(packId, keepWorlds),
    { invalidates: [editorKeys.testSettings(packId)] },
  );
}
