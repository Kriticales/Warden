/**
 * Links do Modrinth e diretos (P1-11): `add_link_modrinth` (consulta que não grava nada) e
 * `add_link_download` (download cancelável numa pasta temporária; o arquivo vira um token para
 * `add_local_inspect` e `add_local_apply`).
 */
import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';

import {
  commands,
  type LocalFileRef,
  type OperationId,
  type PackId,
} from '../../../lib/ipc/bindings';
import { operationChannel } from '../../../lib/ipc/channel';
import { commandQuery } from '../../../lib/ipc/query';
import { CommandError } from '../../../lib/ipc/result';

export function useModrinthLink(packId: PackId, url: string | null) {
  return useQuery({
    ...commandQuery(['modrinthLink', packId, url ?? ''] as const, () =>
      commands.addLinkModrinth(packId, url ?? ''),
    ),
    enabled: url !== null,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export type DownloadState =
  | { status: 'downloading' }
  | { status: 'done'; file: LocalFileRef }
  | { status: 'cancelled' }
  | { status: 'error'; error: CommandError };

/**
 * Baixa o link direto uma vez. Sair da tela (ou "Cancelar download") cancela a operação no
 * backend, que apaga o que já tinha baixado. Quem usa monta um painel por link (`key`).
 */
export function useDirectDownload(packId: PackId, url: string) {
  const [state, setState] = useState<DownloadState>({ status: 'downloading' });
  const [attempt, setAttempt] = useState(0);
  const operation = useRef<OperationId | null>(null);
  const cancelled = useRef(false);

  useEffect(() => {
    let live = true;
    operation.current = null;
    cancelled.current = false;
    const channel = operationChannel((event) => {
      if (event.type === 'started') {
        operation.current = event.operationId;
        if (!live || cancelled.current) void commands.operationCancel(event.operationId);
      }
    });
    void commands.addLinkDownload(packId, url, channel).then((result) => {
      if (!live) {
        // A tela saiu depois de o download terminar: o arquivo baixado não vai ser usado.
        if (result.status === 'ok') void commands.addLocalDiscard([result.data.token]);
        return;
      }
      if (result.status === 'ok') setState({ status: 'done', file: result.data });
      else if (cancelled.current || result.error.code.code === 'CANCELLED')
        setState({ status: 'cancelled' });
      else setState({ status: 'error', error: new CommandError(result.error) });
    });
    return () => {
      live = false;
      if (operation.current !== null) void commands.operationCancel(operation.current);
    };
  }, [packId, url, attempt]);

  const cancel = useCallback(() => {
    cancelled.current = true;
    if (operation.current !== null) void commands.operationCancel(operation.current);
  }, []);
  const retry = useCallback(() => {
    setState({ status: 'downloading' });
    setAttempt((value) => value + 1);
  }, []);
  return { state, cancel, retry };
}
