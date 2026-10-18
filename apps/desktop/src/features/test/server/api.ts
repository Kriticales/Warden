/**
 * Chamadas do servidor local (SPEC T27 e T21; ROADMAP L-09): a conta de memória do diálogo
 * "Testar como servidor", o aceite da EULA, os comandos pelo stdin, o servidor aberto agora
 * (com o evento `server-state`) e o console do servidor de uma sessão gravada.
 *
 * O aceite da EULA mora em `settings.json` (`eulaAcceptedAt`): aceitar e revogar gravam a
 * resposta nova no cache das Configurações e invalidam as contas do servidor de todos os packs,
 * que dizem se a EULA falta.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import {
  commands,
  events,
  type PackId,
  type ServerState,
  type Settings,
} from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { settingsKeys } from '../../settings/api';

export const serverKeys = {
  /** Todas as contas de memória do servidor (de qualquer pack). */
  plans: ['server', 'plan'] as const,
  /** A conta de um pack. */
  plan: (packId: PackId, withGame: boolean) =>
    ['server', 'plan', packId, withGame ? 'jogo' : 'so'] as const,
  /** O servidor aberto agora. */
  current: ['server', 'current'] as const,
  /** O console do servidor de uma sessão gravada (sob o pack). */
  log: (packId: PackId, sessionId: string) =>
    [...queryKeys.pack(packId), 'test', 'session', sessionId, 'server'] as const,
};

/** A conta de memória e os mods que ficam de fora (lida de novo a cada abertura do diálogo). */
export function useServerPlan(packId: PackId, withGame: boolean, enabled: boolean) {
  return useQuery({
    ...commandQuery(serverKeys.plan(packId, withGame), () =>
      commands.serverPlan(packId, null, withGame),
    ),
    enabled,
    staleTime: 0,
  });
}

/** Grava a resposta da EULA nas Configurações e invalida as contas do servidor. */
function useEulaMutation(run: () => ReturnType<typeof commands.serverEulaAccept>) {
  const queryClient = useQueryClient();
  return useCommandMutation(run, {
    invalidates: [serverKeys.plans],
    onSuccess: (settings: Settings) => {
      queryClient.setQueryData(settingsKeys.settings, settings);
    },
  });
}

/** "Aceito a EULA". */
export function useAcceptEula() {
  return useEulaMutation(() => commands.serverEulaAccept());
}

/** "Revogar" (Configurações → Teste). */
export function useRevokeEula() {
  return useEulaMutation(() => commands.serverEulaRevoke());
}

/** "Enviar comando ao servidor". */
export function useSendServerCommand(packId: PackId) {
  return useCommandMutation((command: string) => commands.serverSendCommand(packId, command));
}

/** De quanto em quanto tempo o servidor aberto é relido enquanto muda de fase. */
const TRANSIENT_POLL_MS = 500;

/** Fases que passam sozinhas: o evento seguinte pode ter saído antes de a tela o escutar. */
const TRANSIENT_PHASES: ReadonlySet<ServerState['phase']> = new Set([
  'preparing',
  'starting',
  'stopping',
]);

/**
 * O servidor aberto agora, mantido pelo evento `server-state`. Enquanto a fase for passageira,
 * relê a cada meio segundo: o `listen` do Tauri é assíncrono, e um "pronto" emitido entre a
 * leitura e a assinatura (ou uma leitura que chega depois do evento) deixaria a tela presa em
 * "carregando", com a linha de comando desligada.
 */
export function useServerState(): ServerState | null {
  const queryClient = useQueryClient();
  const current = useQuery({
    queryKey: serverKeys.current,
    queryFn: () => commands.serverCurrent(),
    refetchInterval: (query) =>
      query.state.data && TRANSIENT_PHASES.has(query.state.data.phase) ? TRANSIENT_POLL_MS : false,
  });
  useTauriEvent(events.serverState, (state) => {
    queryClient.setQueryData(serverKeys.current, state.phase === 'exited' ? null : state);
  });
  return current.data ?? null;
}

/** O console do servidor de uma sessão gravada com o jogo e o servidor. */
export function useServerSessionLog(packId: PackId, sessionId: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(serverKeys.log(packId, sessionId), () =>
      commands.serverSessionLog(packId, sessionId),
    ),
    enabled,
    // Uma sessão gravada não muda.
    staleTime: Infinity,
  });
}
