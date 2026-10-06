/**
 * Dados de Meus packs, Criar pack e Abrir pack (ARCHITECTURE §18): queries e mutations sobre
 * os comandos `packs_*`/`pack_*` e o catálogo de versões. Regra do disco como verdade: as
 * mutations invalidam a lista ao terminar, sem atualização otimista.
 */
import { useQuery } from '@tanstack/react-query';

import {
  commands,
  type CreatePack,
  type FolderPurpose,
  type Loader,
  type PackId,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';

export const packsKeys = {
  /** A lista de Meus packs. */
  list: ['packs', 'list'] as const,
  /** Um pack da lista (página do pack). */
  row: (packId: PackId) => [...queryKeys.pack(packId), 'row'] as const,
  createDefaults: ['packs', 'createDefaults'] as const,
  createCheck: (name: string, destination: string | null) =>
    ['packs', 'createCheck', name, destination] as const,
  importPreview: (path: string) => ['packs', 'importPreview', path] as const,
  /** Higiene de um pack registrado (some quando os arquivos do pack mudam). */
  hygiene: (packId: PackId) => [...queryKeys.pack(packId), 'hygiene'] as const,
  minecraftVersions: ['catalog', 'minecraft'] as const,
  loaderVersions: (loader: Loader, minecraft: string) =>
    ['catalog', 'loader', loader, minecraft] as const,
};

/** A lista de Meus packs. `packs_list` não falha no Rust; o IPC ainda pode falhar. */
export const packsListQuery = commandQuery(packsKeys.list, async () => ({
  status: 'ok' as const,
  data: await commands.packsList(),
}));

export function usePacksList() {
  return useQuery(packsListQuery);
}

export function usePack(packId: PackId) {
  return useQuery(commandQuery(packsKeys.row(packId), () => commands.packGet(packId)));
}

export function useCreateDefaults() {
  return useQuery(
    commandQuery(packsKeys.createDefaults, async () => ({
      status: 'ok' as const,
      data: await commands.packCreateDefaults(),
    })),
  );
}

/** Opções da conferência de nome e pasta (etapa 1); o assistente usa com `fetchQuery` também. */
export function createCheckQuery(name: string, destination: string | null) {
  return {
    ...commandQuery(packsKeys.createCheck(name, destination), () =>
      commands.packCreateCheck(name, destination),
    ),
    // A pasta pode ganhar arquivos a qualquer momento: sempre confere de novo.
    staleTime: 0,
    gcTime: 0,
  };
}

export function useMinecraftVersions() {
  return useQuery(
    commandQuery(packsKeys.minecraftVersions, () => commands.catalogMinecraftVersions(false)),
  );
}

export function loaderVersionsQuery(loader: Loader, minecraft: string) {
  return commandQuery(packsKeys.loaderVersions(loader, minecraft), () =>
    commands.catalogLoaderVersions(loader, minecraft, false),
  );
}

export function useHygieneScan(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(packsKeys.hygiene(packId), () => commands.packHygieneScan(packId)),
    enabled,
  });
}

export function useImportPreview(path: string | null) {
  return useQuery({
    ...commandQuery(packsKeys.importPreview(path ?? ''), () =>
      commands.packImportPreview(path ?? ''),
    ),
    enabled: path !== null,
    staleTime: 0,
    gcTime: 0,
  });
}

const invalidatesList = { invalidates: [packsKeys.list] } as const;

export function useCreatePack() {
  return useCommandMutation((request: CreatePack) => commands.packCreate(request), invalidatesList);
}

export function useImportPack() {
  return useCommandMutation(
    ({ path, addControls }: { path: string; addControls: boolean }) =>
      commands.packImport(path, addControls),
    invalidatesList,
  );
}

export function useHygieneFix() {
  return useCommandMutation(
    ({ packId, paths }: { packId: PackId; paths: string[] }) =>
      commands.packHygieneFix(packId, paths),
    { invalidates: [packsKeys.list, ['pack']] },
  );
}

export function useRelocatePack() {
  return useCommandMutation(
    ({ packId, path }: { packId: PackId; path: string }) => commands.packRelocate(packId, path),
    invalidatesList,
  );
}

export function useForgetPack() {
  return useCommandMutation((packId: PackId) => commands.packForget(packId), invalidatesList);
}

export function useTrashPack() {
  return useCommandMutation(
    ({ packId, confirmation }: { packId: PackId; confirmation: string }) =>
      commands.packTrash(packId, confirmation),
    invalidatesList,
  );
}

export function useRevealPack() {
  return useCommandMutation((packId: PackId) => commands.packRevealFolder(packId));
}

/** Abre o diálogo nativo de pasta; `null` = o usuário desistiu. */
export function useChooseFolder() {
  return useCommandMutation((purpose: FolderPurpose) => commands.packChooseFolder(purpose));
}
