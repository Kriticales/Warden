/**
 * Dados de "Mods opcionais e fixar versão" (T11): a tabela `[option]` e o `pin` do item (no
 * pack, pelo inventário) e as escolhas dos opcionais da instância de teste (neste computador).
 *
 * Regra do disco como verdade: sem atualização otimista. As escritas do pack invalidam a área
 * do inventário (e a linha do pack, que mostra as alterações não salvas); as escolhas da
 * instância não mudam o pack e só invalidam a própria query.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type OptionSettings, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { packsKeys } from '../../packs/api';

export const optionKeys = {
  /** A tabela `[option]` de um item (some junto quando o inventário muda). */
  option: (packId: PackId, path: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'option', path] as const,
  /** Os opcionais do pack e a escolha da instância de teste. */
  choices: (packId: PackId) => [...queryKeys.pack(packId), 'optionalChoices'] as const,
};

function packWritten(packId: PackId) {
  return {
    invalidates: [queryKeys.packArea(packId, 'inventory'), packsKeys.row(packId), packsKeys.list],
  } as const;
}

export function useItemOption(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(optionKeys.option(packId, path), () => commands.itemOptionGet(packId, path)),
    enabled,
    staleTime: 0,
  });
}

/** Marca (`settings`) ou desmarca (`null`) o item como opcional. */
export function useSetOptional(packId: PackId, path: string) {
  return useCommandMutation(
    (settings: OptionSettings | null) => commands.itemSetOptional(packId, path, settings),
    packWritten(packId),
  );
}

export function useSetPinned(packId: PackId) {
  return useCommandMutation(
    ({ paths, pinned }: { paths: string[]; pinned: boolean }) =>
      commands.itemsSetPinned(packId, paths, pinned),
    packWritten(packId),
  );
}

export function useOptionalChoices(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(optionKeys.choices(packId), () => commands.instanceOptionalChoicesGet(packId)),
    enabled,
    staleTime: 0,
  });
}

export function useSetOptionalChoices(packId: PackId) {
  return useCommandMutation(
    (choices: Record<string, boolean>) => commands.instanceSetOptionalChoices(packId, choices),
    { invalidates: [optionKeys.choices(packId)] },
  );
}
