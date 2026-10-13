/**
 * Chamadas do console agrupado (D-09): os grupos e os itens já calculados no Rust
 * (`console_grouped`) e as linhas originais de um item ou grupo (`console_grouped_lines`).
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';

import {
  commands,
  type GroupedRequest,
  type LinesRequest,
  type PackId,
} from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';
import { unwrap } from '../../../lib/ipc/result';
import { testKeys } from '../api';

/** Com o jogo aberto, os grupos são relidos a cada segundo (o agrupador lê só o que é novo). */
export const LIVE_REFRESH_MS = 1000;

export const groupedKeys = {
  /** Tudo do console agrupado de uma sessão (sob a sessão do pack). */
  session: (packId: PackId, sessionId: string) =>
    [...testKeys.session(packId, sessionId), 'grouped'] as const,
  view: (packId: PackId, sessionId: string, request: GroupedRequest) =>
    [...groupedKeys.session(packId, sessionId), 'view', request] as const,
  lines: (packId: PackId, sessionId: string, target: LinesRequest, limit: number | null) =>
    [...groupedKeys.session(packId, sessionId), 'lines', target, limit] as const,
};

/** Os grupos (ou "Só problemas") da sessão; relidos a cada segundo com o jogo aberto. */
export function useGroupedConsole(
  packId: PackId,
  sessionId: string | null,
  request: GroupedRequest,
  live: boolean,
) {
  return useQuery({
    ...commandQuery(groupedKeys.view(packId, sessionId ?? '', request), () =>
      commands.consoleGrouped(packId, sessionId ?? '', request),
    ),
    enabled: sessionId !== null,
    refetchInterval: live ? LIVE_REFRESH_MS : false,
    // Abrir um grupo ou digitar na busca não apaga a tela enquanto a resposta chega.
    placeholderData: keepPreviousData,
  });
}

/** As linhas originais de um item (o stack trace aberto), lidas quando o item abre. */
export function useItemLines(packId: PackId, sessionId: string, id: number, enabled: boolean) {
  const target: LinesRequest = { type: 'item', id };
  return useQuery({
    ...commandQuery(groupedKeys.lines(packId, sessionId, target, 1), () =>
      commands.consoleGroupedLines(packId, sessionId, target, 1),
    ),
    enabled,
    // A primeira ocorrência de um item não muda.
    staleTime: Infinity,
  });
}

/** Todas as linhas originais de um grupo ("Copiar"). */
export async function groupLines(packId: PackId, sessionId: string, key: string) {
  return unwrap(
    await commands.consoleGroupedLines(packId, sessionId, { type: 'group', key }, null),
  );
}
