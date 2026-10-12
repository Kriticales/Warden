/**
 * Arquivos do computador (P1-11): escolher no diálogo nativo, ler (metadados e procura pelo
 * hash), gravar e esquecer. A tela só conhece tokens e nomes de arquivo, nunca caminhos.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type LocalApplyItem, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { packsKeys } from '../../packs/api';

/** Lê os arquivos: uma vez por conjunto de tokens, sem cache (o arquivo pode mudar no disco). */
export function useLocalInspect(packId: PackId, tokens: readonly string[]) {
  return useQuery({
    ...commandQuery(['localInspect', packId, ...tokens] as const, () =>
      commands.addLocalInspect(packId, [...tokens]),
    ),
    enabled: tokens.length > 0,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** "Escolher arquivo do computador…" (diálogo nativo; vazio se o usuário desistiu). */
export function useChooseFiles(packId: PackId) {
  return useCommandMutation(() => commands.addLocalChooseFiles(packId));
}

/** Grava os arquivos confirmados; o inventário e a linha do pack são relidos ao terminar. */
export function useLocalApply(packId: PackId) {
  return useCommandMutation((items: LocalApplyItem[]) => commands.addLocalApply(packId, items), {
    invalidates: [queryKeys.packArea(packId, 'inventory'), packsKeys.row(packId), packsKeys.list],
  });
}

/** Esquece arquivos que não vão entrar (apaga o download do link direto). */
export function discardFiles(tokens: readonly string[]): void {
  if (tokens.length > 0) void commands.addLocalDiscard([...tokens]);
}
