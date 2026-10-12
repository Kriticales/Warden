/**
 * "Entrar com o GitHub" (UI-03; ADR-0059): a situação do login e as ações. A espera pela
 * aprovação roda no Rust e avisa pelo evento `github-login-updated`; quando ela termina, a
 * situação é lida de novo. Nenhum token passa por aqui.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { commands, events, type GithubLoginStatus } from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';

export const githubKeys = {
  login: ['app', 'githubLogin'] as const,
};

export const githubLoginQuery = commandQuery(githubKeys.login, commands.githubLoginStatus);

/** Situação do login (disponível, conta, esperando). */
export function useGithubLogin() {
  return useQuery(githubLoginQuery);
}

/** Relê a situação quando a espera termina (conectado, falhou ou cancelado). */
export function useGithubLoginRefresh(): void {
  const queryClient = useQueryClient();
  useTauriEvent(events.githubLoginUpdated, (update) => {
    if (update.state !== 'waiting' && update.state !== 'slowDown') {
      void queryClient.invalidateQueries({ queryKey: githubKeys.login });
    }
  });
}

export function useGithubLoginStart() {
  return useCommandMutation(() => commands.githubLoginStart(), {
    invalidates: [githubKeys.login],
  });
}

export function useGithubLoginCancel() {
  return useCommandMutation(() => commands.githubLoginCancel(), {
    invalidates: [githubKeys.login],
  });
}

export function useGithubLoginOpenPage() {
  return useCommandMutation(() => commands.githubLoginOpenPage());
}

export function useGithubLogout() {
  const queryClient = useQueryClient();
  return useCommandMutation(() => commands.githubLogout(), {
    onSuccess: (status: GithubLoginStatus) => {
      queryClient.setQueryData(githubKeys.login, status);
    },
    // O token do GitHub também aparece em Chaves e contas.
    invalidates: [['app', 'secrets']],
  });
}
