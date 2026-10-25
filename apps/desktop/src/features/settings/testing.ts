/**
 * Backend simulado com estado para os testes de Configurações e da primeira execução: o
 * `settings.json`, as chaves (com o final e o último teste) e a pasta dos packs mudam como no
 * Rust, para os testes acompanharem o fluxo inteiro (salvar → testar → "Funcionando", concluir
 * → Meus packs).
 */
import type {
  EnvMigrationNotice,
  KeyCheckOutcome,
  KeysOverview,
  SecretKind,
  SecretsStatus,
  SecretTestResult,
  Settings,
  SettingsPatch,
  SettingsStatus,
} from '../../lib/ipc/bindings';
import { ipcError, mockBackend, type Handler, type MockBackend } from '../../test/backend';
import { makeAppError } from '../../test/factories';

export const DEFAULT_PACKS_DIR = 'C:\\Users\\teste\\Documents\\Warden';

/** A ordem das chaves em `secrets_overview` (a de `SecretKind::ALL` no Rust). */
export const ALL_SECRET_KINDS: readonly SecretKind[] = [
  'curseforge',
  'gemini',
  'openai',
  'anthropic',
  'github',
];

/** O que o teste da chave responde (`valid`, `notTestable` ou o código de um erro). */
export type FakeTestAnswer =
  | SecretTestResult['status']
  | { domain: 'ai' | 'curseforge' | 'versioning' | 'core'; code: string };

/** Quando os testes da chave rodaram (fixo, para as frases "hoje às…" serem estáveis). */
export const CHECK_AT = '2026-10-25T21:40:00Z';

export function makeSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    schemaVersion: 1,
    packsDir: null,
    playerName: 'Jogador',
    logLevel: 'normal',
    configDiffBeforeSave: true,
    showPrereleaseVersions: false,
    updateCheckIntervalHours: 24,
    testMemory: { mode: 'auto' },
    eulaAcceptedAt: null,
    ...overrides,
  };
}

export interface FakeState {
  settings: Settings;
  /** Se o `settings.json` existe. */
  exists: boolean;
  secrets: SecretsStatus;
  /** O final de cada chave gravada (`••••xxxx`). */
  hints: Partial<Record<SecretKind, string>>;
  /** O último teste de cada chave. */
  checks: Partial<Record<SecretKind, KeyCheckOutcome>>;
  /** O que cada teste responde (padrão: `valid`). */
  answers: Partial<Record<SecretKind, FakeTestAnswer>>;
  migration: EnvMigrationNotice | null;
}

export interface FakeSettingsBackend extends MockBackend {
  state: FakeState;
}

export function noSecrets(): SecretsStatus {
  return { curseforge: false, gemini: false, openai: false, anthropic: false, github: false };
}

export function overviewOf(state: FakeState): KeysOverview {
  return {
    keys: ALL_SECRET_KINDS.map((kind) => {
      const outcome = state.checks[kind];
      return {
        kind,
        configured: state.secrets[kind],
        hint: state.secrets[kind] ? (state.hints[kind] ?? null) : null,
        check: state.secrets[kind] && outcome ? { outcome, at: CHECK_AT } : null,
      };
    }),
    migration: state.migration,
  };
}

export function mockSettingsBackend(
  initial: Partial<FakeState> = {},
  handlers: Record<string, Handler> = {},
): FakeSettingsBackend {
  const state: FakeState = {
    settings: makeSettings(),
    exists: true,
    secrets: noSecrets(),
    hints: {},
    checks: {},
    answers: {},
    migration: null,
    ...initial,
  };
  const status = (): SettingsStatus => ({
    firstRun: !state.exists,
    packsDir: state.settings.packsDir ?? DEFAULT_PACKS_DIR,
    defaultPacksDir: DEFAULT_PACKS_DIR,
  });
  const backend = mockBackend({
    settings_get: () => state.settings,
    settings_status: status,
    settings_update: ({ patch }) => {
      state.settings = { ...state.settings, ...(patch as SettingsPatch) } as Settings;
      state.exists = true;
      return state.settings;
    },
    secrets_status: () => state.secrets,
    secrets_overview: () => overviewOf(state),
    secrets_migration_retry: () => {
      state.migration = null;
      return overviewOf(state);
    },
    secrets_set: ({ kind, value }) => {
      const secret = kind as SecretKind;
      const text = String(value).trim();
      state.secrets = { ...state.secrets, [secret]: true };
      state.hints = { ...state.hints, [secret]: text.slice(-4) };
      state.checks = { ...state.checks, [secret]: undefined };
      return state.secrets;
    },
    secrets_remove: ({ kind }) => {
      state.secrets = { ...state.secrets, [kind as SecretKind]: false };
      state.checks = { ...state.checks, [kind as SecretKind]: undefined };
      return state.secrets;
    },
    secrets_test: ({ kind }) => {
      const secret = kind as SecretKind;
      const answer = state.answers[secret] ?? 'valid';
      if (answer === 'valid') {
        state.checks = { ...state.checks, [secret]: 'working' };
        return { status: 'valid' };
      }
      if (answer === 'notTestable') return { status: 'notTestable' };
      const refusal = answer.code === 'KEY_INVALID' || answer.code === 'GITHUB_TOKEN_REJECTED';
      if (refusal) state.checks = { ...state.checks, [secret]: 'refused' };
      return ipcError(makeAppError(answer as Parameters<typeof makeAppError>[0]));
    },
    logs_reveal_folder: () => null,
    // Armazenamento (A-06): nada baixado ainda.
    storage_usage: () => ({ cache: 0, clearable: 0, minecraft: 0, javas: 0, instances: 0 }),
    ...handlers,
  });
  return Object.assign(backend, { state });
}
