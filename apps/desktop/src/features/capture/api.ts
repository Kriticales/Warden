/**
 * "O que mudou durante o teste" (C-03): a captura do último teste, a procura dos mods
 * colocados à mão, as versões de um arquivo, trazer para o pack e descartar.
 *
 * Dados sempre atuais: trazer muda o pack (configs, `options.txt`, índice), então invalida tudo
 * o que é do pack (linha com "Salvar versão · N alterações", configs, alterações não salvas no
 * Histórico, a própria captura e o aviso do cabeçalho) e a lista de Meus packs. Descartar muda só
 * a captura (o resultado do teste e o aviso do cabeçalho leem a mesma chave).
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type CaptureApplyRequest, type PackId } from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { packsKeys } from '../packs/api';
import { useTestRun } from '../test/store';

export const captureKeys = {
  /** Tudo da captura do pack. */
  all: (packId: PackId) => [...queryKeys.pack(packId), 'capture'] as const,
  /** A captura vista depois do teste `marker` (muda quando um teste termina nesta sessão). */
  view: (packId: PackId, marker: string | null) =>
    [...captureKeys.all(packId), 'view', marker] as const,
  identify: (packId: PackId, sessionId: string) =>
    [...captureKeys.all(packId), 'identify', sessionId] as const,
  texts: (packId: PackId, sessionId: string, path: string) =>
    [...captureKeys.all(packId), 'texts', sessionId, path] as const,
};

/** O teste deste pack que terminou por último nesta sessão do app (relê a captura). */
function useRunMarker(packId: PackId): string | null {
  const run = useTestRun(packId);
  if (!run) return null;
  return run.result?.id ?? `${run.status}-${String(run.startedAtMs)}`;
}

/** A captura do último teste do pack (`null`: nenhuma). */
export function useCapture(packId: PackId) {
  const marker = useRunMarker(packId);
  return useQuery({
    ...commandQuery(captureKeys.view(packId, marker), () => commands.captureGet(packId)),
  });
}

/** Mods, resource packs e shaders colocados à mão, procurados pelo hash. */
export function useCaptureIdentify(packId: PackId, sessionId: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(captureKeys.identify(packId, sessionId), () =>
      commands.captureIdentify(packId),
    ),
    enabled,
    retry: false,
    staleTime: Infinity,
  });
}

/** As três versões de um arquivo (início do teste, pack agora, jogo agora). */
export function useCaptureTexts(packId: PackId, sessionId: string, path: string, enabled = true) {
  return useQuery({
    ...commandQuery(captureKeys.texts(packId, sessionId, path), () =>
      commands.captureTexts(packId, sessionId, path),
    ),
    enabled,
    staleTime: 0,
  });
}

/** "Trazer selecionados para o pack". */
export function useCaptureApply(packId: PackId) {
  return useCommandMutation(
    (request: CaptureApplyRequest) => commands.captureApply(packId, request),
    { invalidates: [queryKeys.pack(packId), packsKeys.list] },
  );
}

/** "Descartar". */
export function useCaptureDiscard(packId: PackId) {
  return useCommandMutation((sessionId: string) => commands.captureDiscard(packId, sessionId), {
    invalidates: [captureKeys.all(packId)],
  });
}
