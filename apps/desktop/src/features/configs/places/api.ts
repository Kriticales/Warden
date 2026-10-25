/**
 * Dados da seção Configs com as três origens juntas (SPEC T12; ADR-0067): a lista juntada
 * (`config_overview`), as diferenças de um arquivo entre o teste e o pack (`config_compare`),
 * "Trazer para o pack" (`config_bring`), "Voltar ao do pack" (`config_revert`) e as configs
 * pessoais (`config_mark_personal`, `config_unmark_personal`, `config_keep_personal_out`).
 *
 * Regra do disco como verdade, como o resto da seção: a lista e a comparação são relidas ao
 * abrir a seção e ao voltar para a janela; as ações invalidam tudo de configs do pack (a lista,
 * os arquivos abertos, a busca) e a linha do pack (alterações não salvas).
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { packsKeys } from '../../packs/api';
import { configsKeys } from '../api';

export const placesKeys = {
  overview: (packId: PackId) => [...configsKeys.all(packId), 'overview'] as const,
  compare: (packId: PackId, path: string) => [...configsKeys.all(packId), 'compare', path] as const,
};

const fromDisk = {
  staleTime: 0,
  refetchOnMount: 'always',
  refetchOnWindowFocus: 'always',
} as const;

export function useConfigOverview(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(placesKeys.overview(packId), () => commands.configOverview(packId)),
    ...fromDisk,
    enabled,
  });
}

export function useConfigCompare(packId: PackId, path: string | null, enabled = true) {
  return useQuery({
    ...commandQuery(placesKeys.compare(packId, path ?? ''), () =>
      commands.configCompare(packId, path ?? ''),
    ),
    ...fromDisk,
    enabled: enabled && path !== null,
  });
}

/** Tudo de configs do pack, a linha e a lista de Meus packs (alterações não salvas). */
function configsWide(packId: PackId) {
  return [configsKeys.all(packId), packsKeys.row(packId), packsKeys.list];
}

export interface CopyRequest {
  path: string;
  /** Só estas chaves; sem elas, o arquivo inteiro. */
  keys?: string[] | null;
}

/** "Trazer para o pack": grava no pack o arquivo do teste (ou só as chaves). */
export function useBring(packId: PackId) {
  return useCommandMutation(
    ({ path, keys }: CopyRequest) => commands.configBring(packId, path, keys ?? null),
    { invalidates: configsWide(packId) },
  );
}

/** "Voltar ao do pack": grava no teste o arquivo do pack (ou só as chaves). */
export function useRevert(packId: PackId) {
  return useCommandMutation(
    ({ path, keys }: CopyRequest) => commands.configRevert(packId, path, keys ?? null),
    { invalidates: configsWide(packId) },
  );
}

/** "Marcar como pessoal" (se estava no pack, sai dele). */
export function useMarkPersonal(packId: PackId) {
  return useCommandMutation((path: string) => commands.configMarkPersonal(packId, path), {
    invalidates: [queryKeys.pack(packId), packsKeys.row(packId), packsKeys.list],
  });
}

/** "Deixar de ser pessoal". */
export function useUnmarkPersonal(packId: PackId) {
  return useCommandMutation((path: string) => commands.configUnmarkPersonal(packId, path), {
    invalidates: configsWide(packId),
  });
}

/** A migração do ADR-0067: tira do pack as configs pessoais, com a cópia guardada aqui. */
export function useKeepPersonalOut(packId: PackId) {
  return useCommandMutation((paths: string[]) => commands.configKeepPersonalOut(packId, paths), {
    invalidates: [queryKeys.pack(packId), packsKeys.row(packId), packsKeys.list],
  });
}
