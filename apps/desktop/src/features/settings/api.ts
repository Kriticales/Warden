/**
 * Dados de Configurações (T21) e da primeira execução (T01): consultas e mutações sobre os
 * comandos `settings_*`, `secrets_*` e `logs_reveal_folder`.
 *
 * O backend devolve o estado novo em cada mutação; ele é gravado direto no cache, sem nova
 * leitura. As chaves nunca passam por aqui depois de salvas: a interface só conhece
 * `SecretsStatus` (ARCHITECTURE §14).
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import {
  commands,
  type AppError,
  type BackendKind,
  type SecretKind,
  type SecretsStatus,
  type Settings,
  type SettingsPatch,
  type SettingsStatus,
} from '../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';

/** O que a IA guarda no app (`aiKeys.app`, repetido para não importar a seção da IA aqui). */
export const AI_APP_KEY = ['app', 'ai'] as const;

export const settingsKeys = {
  settings: ['app', 'settings'] as const,
  status: ['app', 'settings', 'status'] as const,
  secrets: ['app', 'secrets'] as const,
};

export const settingsQuery = commandQuery(settingsKeys.settings, commands.settingsGet);
export const settingsStatusQuery = commandQuery(settingsKeys.status, commands.settingsStatus);
export const secretsStatusQuery = commandQuery(settingsKeys.secrets, commands.secretsStatus);

export function useSettings() {
  return useQuery(settingsQuery);
}

export function useSettingsStatus() {
  return useQuery(settingsStatusQuery);
}

export function useSecretsStatus() {
  return useQuery(secretsStatusQuery);
}

/** Altera configurações (`SettingsPatch`). Gravar também encerra a primeira execução. */
export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useCommandMutation((patch: SettingsPatch) => commands.settingsUpdate(patch), {
    invalidates: [settingsKeys.status],
    onSuccess: (settings: Settings) => {
      queryClient.setQueryData(settingsKeys.settings, settings);
    },
  });
}

/** Abre o diálogo nativo de pasta; `null` quando o usuário desiste. */
export function useChoosePacksDir() {
  const queryClient = useQueryClient();
  return useCommandMutation(() => commands.settingsChoosePacksDir(), {
    onSuccess: async (status: SettingsStatus | null) => {
      if (status) {
        queryClient.setQueryData(settingsKeys.status, status);
        // Só o `settings.json`: a chave do estado começa igual e já está atualizada.
        await queryClient.invalidateQueries({ queryKey: settingsKeys.settings, exact: true });
      }
    },
  });
}

function useSecretsMutation<V>(
  command: (variables: V) => ReturnType<typeof commands.secretsStatus>,
) {
  const queryClient = useQueryClient();
  return useCommandMutation(command, {
    onSuccess: async (status: SecretsStatus) => {
      queryClient.setQueryData(settingsKeys.secrets, status);
      // As chaves dos provedores mudam o "tem chave" e os modelos da IA (D-18).
      await queryClient.invalidateQueries({ queryKey: AI_APP_KEY });
    },
    // Uma falha no meio pode ter mudado parte do estado: relê do disco (regra do disco como
    // verdade).
    onError: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: settingsKeys.secrets }),
        queryClient.invalidateQueries({ queryKey: AI_APP_KEY }),
      ]),
  });
}

export function useSetSecret() {
  return useSecretsMutation(({ kind, value }: { kind: SecretKind; value: string }) =>
    commands.secretsSet(kind, value),
  );
}

export function useRemoveSecret() {
  return useSecretsMutation((kind: SecretKind) => commands.secretsRemove(kind));
}

export function useSetSecretsBackend() {
  return useSecretsMutation((backend: BackendKind) => commands.secretsBackendSet(backend));
}

export function useTestSecret() {
  return useCommandMutation((kind: SecretKind) => commands.secretsTest(kind));
}

export function useRevealLogsFolder() {
  return useCommandMutation(() => commands.logsRevealFolder());
}

/** Regra do nome do jogador no perfil offline (a mesma do `SettingsPatch` no Rust). */
export const PLAYER_NAME_PATTERN = /^[A-Za-z0-9_]{3,16}$/;

export function isValidPlayerName(name: string): boolean {
  return PLAYER_NAME_PATTERN.test(name);
}

/** Motivo de uma pasta recusada por `settings_choose_packs_dir`, quando for o caso. */
export function packsDirProblem(error: AppError | null): string | null {
  if (
    error?.code.domain === 'app' &&
    error.code.code === 'SETTINGS_INVALID' &&
    error.params.field === 'packsDir'
  ) {
    return error.params.reason ?? null;
  }
  return null;
}

/**
 * As chaves das linhas próprias de "Chaves e contas" e da primeira execução, na ordem da tela.
 * As dos provedores da IA (Gemini, OpenAI e Claude; ADR-0060) ficam na área IA da mesma seção
 * (`ai/AiSettings.tsx`) e no passo da IA das boas-vindas (D-18).
 */
export type OwnSecretKind = Extract<SecretKind, 'curseforge' | 'github'>;

export const SECRET_KINDS: readonly OwnSecretKind[] = ['curseforge', 'github'];
