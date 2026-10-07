/**
 * Dados de Salvar versão e do Histórico (ARCHITECTURE §18): queries e mutations sobre
 * `history_*`, `version_*`, `safety_point*` e `unsaved_discard`.
 *
 * Regra do disco como verdade (SPEC T05): sem atualização otimista. Toda escrita invalida o
 * pack inteiro (a linha do pack, com o contador de alterações não salvas, a lista de Mods e o
 * histórico); o `pack-changed` do Rust faz o mesmo para o que mudar por fora.
 */
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import { commands, type PackId, type SaveRequest, type SavedVersion } from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation, type CommandMutationOptions } from '../../lib/ipc/query';
import { packsKeys } from '../packs/api';
import { utcOffsetMinutes } from './model';

export const versioningKeys = {
  /** Alterações não salvas e versões salvas. */
  history: (packId: PackId) => [...queryKeys.packArea(packId, 'history'), 'view'] as const,
  /** O que mudou desde uma versão salva até agora. */
  changes: (packId: PackId, version: string) =>
    [...queryKeys.packArea(packId, 'history'), 'changes', version] as const,
  /** O diálogo Salvar versão: sugestão e changelog automático. */
  preview: (packId: PackId) => [...queryKeys.packArea(packId, 'history'), 'preview'] as const,
  /** Conferência do número da versão digitado. */
  validate: (packId: PackId, version: string) =>
    [...queryKeys.packArea(packId, 'history'), 'validate', version] as const,
  /** Pontos de segurança. */
  safety: (packId: PackId) => [...queryKeys.packArea(packId, 'history'), 'safety'] as const,
};

/** Sempre relido do disco ao abrir a seção e ao voltar para a janela. */
const fromDisk = {
  staleTime: 0,
  refetchOnMount: 'always',
  refetchOnWindowFocus: 'always',
} as const;

export function useHistory(packId: PackId) {
  return useQuery({
    ...commandQuery(versioningKeys.history(packId), () => commands.historyGet(packId)),
    ...fromDisk,
  });
}

/** O que mudou da versão escolhida até o estado atual (só busca com o diálogo aberto). */
export function useVersionChanges(packId: PackId, version: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(versioningKeys.changes(packId, version), () =>
      commands.versionChanges(packId, version),
    ),
    enabled,
    staleTime: 0,
    gcTime: 0,
  });
}

/** Sugestão e changelog automático, calculados de novo a cada vez que o diálogo abre. */
export function useSavePreview(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(versioningKeys.preview(packId), () => commands.versionSavePreview(packId)),
    enabled,
    staleTime: 0,
    gcTime: 0,
  });
}

/** Valor que só muda depois de `delay` ms sem mudar (para não conferir a cada tecla). */
export function useDebouncedValue<T>(value: T, delay = 250): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebounced(value);
    }, delay);
    return () => {
      clearTimeout(timer);
    };
  }, [value, delay]);
  return debounced;
}

/**
 * Confere se o número pode ser salvo (CA-T16-04). O erro traz a explicação (menor ou igual à
 * última versão, número que não é versão, versão repetida).
 */
export function useVersionValidation(packId: PackId, version: string, enabled: boolean) {
  const trimmed = version.trim();
  return useQuery({
    ...commandQuery(versioningKeys.validate(packId, trimmed), () =>
      commands.versionValidate(packId, trimmed),
    ),
    enabled: enabled && trimmed !== '',
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export function useSafetyPoints(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(versioningKeys.safety(packId), () => commands.safetyPointsList(packId)),
    enabled,
    staleTime: 0,
    gcTime: 0,
  });
}

/** Tudo o que uma escrita no histórico pode mudar. */
function invalidates(packId: PackId) {
  return [queryKeys.pack(packId), packsKeys.list] as const;
}

export type SaveInput = Omit<SaveRequest, 'utcOffsetMinutes'>;

/**
 * Salvar a versão. Quem chama deve guardar a mutation num componente que **não** desmonta ao
 * salvar: o histórico muda, a prévia do diálogo é relida ("Nada mudou") e o formulário sai da
 * tela antes de os callbacks de `mutate()` rodarem; os de `options` rodam sempre.
 */
export function useSaveVersion(
  packId: PackId,
  options: Omit<CommandMutationOptions<SavedVersion, SaveInput>, 'invalidates'> = {},
) {
  return useCommandMutation(
    (input: SaveInput) =>
      commands.versionSave(packId, { ...input, utcOffsetMinutes: utcOffsetMinutes() }),
    { ...options, invalidates: invalidates(packId) },
  );
}

export function useSetFinal(packId: PackId) {
  return useCommandMutation(
    ({ version, isFinal }: { version: string; isFinal: boolean }) =>
      commands.versionSetFinal(packId, version, isFinal),
    { invalidates: invalidates(packId) },
  );
}

export function useRestoreVersion(packId: PackId) {
  return useCommandMutation(
    (version: string) => commands.versionRestore(packId, version, utcOffsetMinutes()),
    { invalidates: invalidates(packId) },
  );
}

export function useRecoverSafetyPoint(packId: PackId) {
  return useCommandMutation(
    (name: string) => commands.safetyPointRecover(packId, name, utcOffsetMinutes()),
    { invalidates: invalidates(packId) },
  );
}

export function useDiscardFile(packId: PackId) {
  return useCommandMutation((path: string) => commands.unsavedDiscard(packId, path), {
    invalidates: invalidates(packId),
  });
}
