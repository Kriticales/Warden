/**
 * Dados do editor de scripts (ARCHITECTURE §18; SPEC T26).
 *
 * - `scripts_info` e `scripts_ids` dependem dos mods do pack: ficam na área `inventory`, então
 *   adicionar, remover ou atualizar um mod (`pack-changed`) os relê. A versão (`scripts_info`,
 *   barata) é relida do disco a cada arquivo aberto; os IDs, só pelo evento ou depois de um
 *   minuto.
 * - `scripts_problems` e `scripts_reload_route` dependem do teste: a chave leva a situação do
 *   jogo deste pack, e cada mudança (abriu, fechou) os relê. Com o KubeJS 7 e o jogo aberto, os
 *   erros vêm ao vivo e são relidos a cada poucos segundos.
 * - Recarregar relê os erros (o jogo acabou de rodar os scripts de novo).
 * - Copiar um script salvo no pack para a instância de teste relê tudo o que mostra a instância:
 *   a lista e os arquivos de Configs (origem Instância) e "O que mudou durante o teste".
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { captureKeys } from '../../capture/api';
import { useGame } from '../../test/store';
import { configsKeys } from '../api';

/** De quanto em quanto tempo os erros ao vivo são relidos. */
export const LIVE_PROBLEMS_INTERVAL_MS = 5000;

export const scriptsKeys = {
  info: (packId: PackId) =>
    [...queryKeys.packArea(packId, 'inventory'), 'scripts', 'info'] as const,
  ids: (packId: PackId) => [...queryKeys.packArea(packId, 'inventory'), 'scripts', 'ids'] as const,
  /** Erros e caminho de recarga: dependem do teste. */
  test: (packId: PackId) => ['pack', packId, 'scripts-test'] as const,
  problems: (packId: PackId, game: string) =>
    [...scriptsKeys.test(packId), 'problems', game] as const,
  route: (packId: PackId, path: string, game: string) =>
    [...scriptsKeys.test(packId), 'route', path, game] as const,
};

/** A situação do jogo deste pack, como parte das chaves (`none`, `preparing`, `running`…). */
export function useGameKey(packId: PackId): string {
  const game = useGame();
  return game?.packId === packId ? game.state : 'none';
}

export function useScriptsInfo(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(scriptsKeys.info(packId), () => commands.scriptsInfo(packId)),
    enabled,
    staleTime: 0,
    refetchOnMount: 'always',
  });
}

/** Os IDs do autocompletar: lidos uma vez por arquivo aberto, filtrados na memória. */
export function useScriptIds(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(scriptsKeys.ids(packId), () => commands.scriptsIds(packId)),
    enabled,
    staleTime: 60_000,
  });
}

export function useScriptProblems(packId: PackId, enabled: boolean, live: boolean) {
  const game = useGameKey(packId);
  return useQuery({
    ...commandQuery(scriptsKeys.problems(packId, game), () => commands.scriptsProblems(packId)),
    enabled,
    staleTime: 0,
    refetchOnWindowFocus: 'always',
    refetchInterval: live && game === 'running' ? LIVE_PROBLEMS_INTERVAL_MS : false,
  });
}

export function useReloadRoute(packId: PackId, path: string | null) {
  const game = useGameKey(packId);
  return useQuery({
    ...commandQuery(scriptsKeys.route(packId, path ?? '', game), () =>
      commands.scriptsReloadRoute(packId, path ?? ''),
    ),
    enabled: path !== null,
    staleTime: 0,
  });
}

/** "Recarregar no jogo". */
export function useReloadScripts(packId: PackId) {
  return useCommandMutation((path: string) => commands.scriptsReload(packId, path), {
    invalidates: [scriptsKeys.test(packId)],
  });
}

/** "Abrir no VS Code". */
export function useOpenVsCode(packId: PackId) {
  return useCommandMutation(() => commands.scriptsOpenVscode(packId));
}

export interface CopyRequest {
  path: string;
  /** `hash` do arquivo do pack antes de salvar. */
  previousHash: string;
  /** A pessoa escolheu substituir o texto da instância. */
  overwrite: boolean;
}

/** Copia para a instância de teste o script que acabou de ser salvo no pack. */
export function useCopyToInstance(packId: PackId) {
  return useCommandMutation(
    ({ path, previousHash, overwrite }: CopyRequest) =>
      commands.scriptsCopyToInstance(packId, path, previousHash, overwrite),
    { invalidates: [configsKeys.all(packId), captureKeys.all(packId)] },
  );
}
