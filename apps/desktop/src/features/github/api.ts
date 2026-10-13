/**
 * Dados da publicação para os jogadores (SPEC T18; ARCHITECTURE §11.1 e §18): `github_status`,
 * `github_setup`, `publish_plan`, `publish_run` e `publish_retry_release`.
 *
 * Regra do disco como verdade (SPEC T05): toda escrita invalida o pack inteiro (a linha do pack,
 * o Histórico com o estado "Publicada", a área de publicação, a lista de Mods e as contagens) e a
 * lista de Meus packs, não só a área atual.
 */
import { useQuery } from '@tanstack/react-query';

import {
  commands,
  type AddApplyRequest,
  type GithubSetup,
  type PackId,
  type PublishRunRequest,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { packsKeys } from '../packs/api';
import { utcOffsetMinutes } from '../versioning/model';

export const githubKeys = {
  /** Repositório, versões publicadas e link do pack. */
  status: (packId: PackId) => [...queryKeys.packArea(packId, 'history'), 'github'] as const,
  /**
   * O diálogo "Publicar a versão X": avisos, notas e a situação do GitHub. Fica fora da chave
   * do pack de propósito: conferir de novo exporta a versão, e o diálogo pede isso só quando
   * algo que ele mostra mudou (troca pelo Modrinth, limpeza).
   */
  plan: (packId: PackId, version: string) => ['publishPlan', packId, version] as const,
};

/** Tudo o que uma escrita da publicação pode mudar. */
function invalidates(packId: PackId) {
  return [queryKeys.pack(packId), packsKeys.list] as const;
}

export function useGithubStatus(packId: PackId) {
  return useQuery({
    ...commandQuery(githubKeys.status(packId), () => commands.githubStatus(packId)),
    staleTime: 0,
    refetchOnWindowFocus: 'always',
  });
}

/** Avisos e notas, calculados de novo a cada vez que o diálogo abre. */
export function usePublishPlan(packId: PackId, version: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(githubKeys.plan(packId, version), () => commands.publishPlan(packId, version)),
    enabled,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export function useGithubSetup(packId: PackId) {
  return useCommandMutation((setup: GithubSetup) => commands.githubSetup(packId, setup), {
    invalidates: invalidates(packId),
  });
}

export type PublishInput = Omit<PublishRunRequest, 'utcOffsetMinutes'>;

/**
 * Publicar. Quem chama guarda a mutation num componente que não desmonta ao publicar: o
 * Histórico é relido e o botão da versão some ("Publicada").
 */
export function usePublishRun(packId: PackId) {
  return useCommandMutation(
    (input: PublishInput) =>
      commands.publishRun(packId, {
        ...input,
        utcOffsetMinutes: utcOffsetMinutes(),
      }),
    { invalidates: invalidates(packId) },
  );
}

export function useRetryRelease(packId: PackId) {
  return useCommandMutation((version: string) => commands.publishRetryRelease(packId, version), {
    invalidates: invalidates(packId),
  });
}

/**
 * "Trocar pelo Modrinth": o mesmo `add_apply` do Adicionar, substituindo o metafile da
 * CurseForge. Muda o pack (alterações não salvas, lista de Mods, contagens).
 */
export function useSwapToModrinth(packId: PackId) {
  return useCommandMutation((request: AddApplyRequest) => commands.addApply(packId, request), {
    invalidates: invalidates(packId),
  });
}
