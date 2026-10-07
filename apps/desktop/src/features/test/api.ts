/**
 * Chamadas do Testar (ARCHITECTURE §4.1, domínio `test`): sessões gravadas, console de uma
 * sessão, instância de teste e as ligações globais (`game-state`, `pack-changed` com o jogo
 * aberto, `game-quit-requested`).
 */
import { useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { commands, events, type PackId } from '../../lib/ipc/bindings';
import { useTauriEvent } from '../../lib/ipc/events';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { unwrap } from '../../lib/ipc/result';
import { log } from '../../lib/log';
import { isActive, useTestStore } from './store';

/** Tipo da operação do teste (`OperationKind` no Rust). */
export const TEST_OPERATION = 'test.start';

export const testKeys = {
  /** As sessões gravadas do pack (sob o pack: some quando o pack é esquecido). */
  sessions: (packId: PackId) => [...queryKeys.pack(packId), 'test', 'sessions'] as const,
  /** Uma sessão com o console. */
  session: (packId: PackId, sessionId: string) =>
    [...queryKeys.pack(packId), 'test', 'session', sessionId] as const,
  /** Os mundos da instância de teste. */
  worlds: (packId: PackId) => [...queryKeys.pack(packId), 'test', 'worlds'] as const,
};

export function useTestSessions(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(testKeys.sessions(packId), () => commands.testSessionsList(packId)),
    enabled,
  });
}

export function useTestSession(packId: PackId, sessionId: string | null) {
  return useQuery({
    ...commandQuery(testKeys.session(packId, sessionId ?? ''), () =>
      commands.testSessionGet(packId, sessionId ?? ''),
    ),
    enabled: sessionId !== null,
    // Uma sessão gravada não muda.
    staleTime: Infinity,
  });
}

export function useInstanceWorlds(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(testKeys.worlds(packId), () => commands.instanceWorldsList(packId)),
    enabled,
    staleTime: 0,
  });
}

export function useDeleteWorlds(packId: PackId) {
  return useCommandMutation(() => commands.instanceDeleteWorlds(packId), {
    invalidates: [testKeys.worlds(packId), [...queryKeys.pack(packId), 'testSettings']],
  });
}

export function useRevealInstance(packId: PackId) {
  return useCommandMutation(() => commands.instanceRevealFolder(packId));
}

export function useSaveSessionLog(packId: PackId) {
  return useCommandMutation((sessionId: string) => commands.testSessionSaveLog(packId, sessionId));
}

export function useOpenArtifact(packId: PackId) {
  return useCommandMutation(({ sessionId, path }: { sessionId: string; path: string }) =>
    commands.testArtifactOpen(packId, sessionId, path),
  );
}

/** Depois que um teste termina, a lista de sessões e a linha do pack (último teste) mudam. */
export function invalidateAfterTest(queryClient: QueryClient, packId: PackId): Promise<void> {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: testKeys.sessions(packId) }),
    queryClient.invalidateQueries({ queryKey: ['packs'] }),
    queryClient.invalidateQueries({ queryKey: [...queryKeys.pack(packId), 'testSettings'] }),
  ]).then(() => undefined);
}

/**
 * Liga o Testar ao backend, uma vez na raiz do app: o `game-state` (com a leitura inicial, para
 * a interface se reencontrar com um jogo aberto depois de recarregar) e o aviso "o pack mudou
 * desde o início do teste" pelo `pack-changed`.
 */
export function useGameStateSync(): void {
  const queryClient = useQueryClient();
  const setGame = useTestStore((store) => store.setGame);
  const notePackChanged = useTestStore((store) => store.notePackChanged);

  useEffect(() => {
    let cancelled = false;
    const reading = commands.testGameState();
    reading
      .then((state) => {
        if (!cancelled && state) {
          setGame(state);
        }
      })
      .catch((error: unknown) => {
        log.error('falha ao ler o jogo aberto', error);
      });
    return () => {
      cancelled = true;
    };
  }, [setGame]);

  useTauriEvent(events.gameState, (state) => {
    setGame(state);
    if (state.state === 'exited') {
      void invalidateAfterTest(queryClient, state.packId);
    }
  });
  useTauriEvent(events.packChanged, (change) => {
    notePackChanged(change.packId);
  });
}

/**
 * Reencontra o console do teste aberto quando a interface recarregou no meio dele (sem o
 * canal do `test_start`): lê as linhas do backend a cada segundo enquanto o jogo estiver
 * aberto.
 */
export function useAdoptedConsole(packId: PackId): void {
  const game = useTestStore((store) => store.game);
  const run = useTestStore((store) => store.runs[packId]);
  const adopt = useTestStore((store) => store.adoptLiveConsole);
  const gameHere = game?.packId === packId;
  const needsAdoption = gameHere && (!run || (run.adopted && isActive(run)));

  useEffect(() => {
    if (!needsAdoption) {
      return undefined;
    }
    let cancelled = false;
    const read = () => {
      const reading = commands.testLiveConsole(packId);
      reading
        .then((lines) => {
          if (!cancelled) adopt(packId, lines);
        })
        .catch((error: unknown) => {
          log.error('falha ao ler o console do teste', error);
        });
    };
    read();
    const timer = window.setInterval(read, 1000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [needsAdoption, packId, adopt]);
}

/** "Fechar o Warden" confirmado com o jogo aberto. */
export async function quitWithGame(): Promise<void> {
  unwrap(await commands.testQuitApp());
}
