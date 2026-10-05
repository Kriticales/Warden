/**
 * Backend simulado com estado para os testes de Configurações e da primeira execução: o
 * `settings.json`, o estado das chaves e a pasta dos packs mudam como no Rust, para os testes
 * acompanharem o fluxo inteiro (salvar → "Configurada", concluir → Meus packs).
 */
import type {
  BackendKind,
  SecretKind,
  SecretsStatus,
  Settings,
  SettingsPatch,
  SettingsStatus,
} from '../../lib/ipc/bindings';
import { mockBackend, type Handler, type MockBackend } from '../../test/backend';

export const DEFAULT_PACKS_DIR = 'C:\\Users\\teste\\Documents\\Warden';

export function makeSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    schemaVersion: 1,
    secretsBackend: 'keyring',
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
}

export interface FakeSettingsBackend extends MockBackend {
  state: FakeState;
}

export function mockSettingsBackend(
  initial: Partial<FakeState> = {},
  handlers: Record<string, Handler> = {},
): FakeSettingsBackend {
  const state: FakeState = {
    settings: makeSettings(),
    exists: true,
    secrets: { backend: 'keyring', curseforge: false, gemini: false, github: false },
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
    secrets_set: ({ kind }) => {
      state.secrets = { ...state.secrets, [kind as SecretKind]: true };
      return state.secrets;
    },
    secrets_remove: ({ kind }) => {
      state.secrets = { ...state.secrets, [kind as SecretKind]: false };
      return state.secrets;
    },
    secrets_backend_set: ({ backend: next }) => {
      state.secrets = { ...state.secrets, backend: next as BackendKind };
      state.settings = { ...state.settings, secretsBackend: next as BackendKind };
      state.exists = true;
      return state.secrets;
    },
    secrets_test: () => ({ status: 'valid' }),
    logs_reveal_folder: () => null,
    ...handlers,
  });
  return Object.assign(backend, { state });
}
