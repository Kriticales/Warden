/**
 * Dados e backend simulado dos perfis do teste (L-08) para os testes de componente: um
 * `packs.json` em memória com o Padrão e os perfis criados, que responde a `test_profile*` como
 * o Rust (o Padrão não pode ser renomeado nem apagado; apagar o ativo volta para o Padrão).
 */
import type { ProfileFields, TestProfile, TestProfilesView } from '../../../lib/ipc/bindings';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { DEFAULT_PROFILE } from './text';

export function makeProfileFields(overrides: Partial<ProfileFields> = {}): ProfileFields {
  return {
    memory: { mode: 'auto' },
    java: null,
    jvmArgs: '',
    optionals: null,
    window: { mode: 'default' },
    onLaunch: { mode: 'menu' },
    ...overrides,
  };
}

export function makeProfile(
  id: string,
  name: string,
  overrides: Partial<ProfileFields> = {},
): TestProfile {
  return { id, name, ...makeProfileFields(overrides) };
}

export function makeProfilesView(overrides: Partial<TestProfilesView> = {}): TestProfilesView {
  return {
    active: DEFAULT_PROFILE,
    profiles: [makeProfile(DEFAULT_PROFILE, '')],
    computerMemoryMb: 16_384,
    minecraft: '1.20.1',
    quickPlay: 'supported',
    ...overrides,
  };
}

/** Os comandos dos perfis sobre um estado em memória (começa em `initial`). */
export function profilesHandlers(initial: TestProfilesView = makeProfilesView()) {
  const view: TestProfilesView = structuredClone(initial);
  let next = view.profiles.length;
  const find = (id: unknown) => view.profiles.find((profile) => profile.id === id);
  const handlers: Record<string, Handler> = {
    test_profiles_get: () => structuredClone(view),
    test_profile_create: ({ name, fields }) => {
      const profile: TestProfile = {
        id: `perfil-${String(next++)}`,
        name: String(name).trim(),
        ...(fields as ProfileFields),
      };
      view.profiles.push(profile);
      return structuredClone(profile);
    },
    test_profile_rename: ({ profileId, name }) => {
      const profile = find(profileId);
      if (!profile || profile.id === DEFAULT_PROFILE) {
        ipcError(makeAppError({ domain: 'app', code: 'TEST_PROFILE_DEFAULT_LOCKED' }));
      }
      profile.name = String(name).trim();
      return null;
    },
    test_profile_delete: ({ profileId }) => {
      view.profiles = view.profiles.filter((profile) => profile.id !== profileId);
      if (view.active === profileId) view.active = DEFAULT_PROFILE;
      return null;
    },
    test_profile_select: ({ profileId }) => {
      view.active = String(profileId);
      return null;
    },
    test_profiles_save: ({ profiles }) => {
      for (const changed of profiles as TestProfile[]) {
        const index = view.profiles.findIndex((profile) => profile.id === changed.id);
        if (index >= 0) view.profiles[index] = structuredClone(changed);
      }
      return structuredClone(view);
    },
  };
  return { view, handlers };
}
