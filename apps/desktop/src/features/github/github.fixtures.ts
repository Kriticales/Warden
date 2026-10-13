/**
 * Dados de teste da publicação (V-03): estado do GitHub, plano e resultado, no formato do
 * `bindings.ts`.
 */
import type {
  GithubLinkView,
  GithubStatus,
  PublishPlan,
  Published,
  PublishedVersion,
} from '../../lib/ipc/bindings';

export function makeGithubLink(overrides: Partial<GithubLinkView> = {}): GithubLinkView {
  return {
    repo: { owner: 'kriticales', name: 'vale-sereno' },
    private: false,
    repoUrl: 'https://github.com/kriticales/vale-sereno',
    packLink: 'https://raw.githubusercontent.com/kriticales/vale-sereno/main/pack.toml',
    ...overrides,
  };
}

export function makePublishedVersion(
  version: string,
  overrides: Partial<PublishedVersion> = {},
): PublishedVersion {
  return { version, date: '2026-09-20T10:00:00-03:00', releasePending: false, ...overrides };
}

/** Pack nunca publicado, com o token salvo. */
export function makeGithubStatus(overrides: Partial<GithubStatus> = {}): GithubStatus {
  return {
    link: null,
    published: [],
    tokenConfigured: true,
    suggestedName: 'vale-sereno',
    minecraft: '1.20.1',
    loader: 'Forge 47.3.0',
    ...overrides,
  };
}

export function makePublishPlan(overrides: Partial<PublishPlan> = {}): PublishPlan {
  return {
    version: '1.5.0',
    lastPublished: '1.4.0',
    notes: '### Mods adicionados\n- Sophisticated Backpacks 3.20.17\n',
    findings: [],
    blocked: false,
    files: 12,
    link: makeGithubLink(),
    tokenConfigured: true,
    remote: { remoteMain: 'a'.repeat(40), inSync: true },
    ...overrides,
  };
}

export function makePublished(overrides: Partial<Published> = {}): Published {
  return {
    version: '1.5.0',
    packLink: 'https://raw.githubusercontent.com/kriticales/vale-sereno/main/pack.toml',
    release: { id: 1234, htmlUrl: 'https://github.com/kriticales/vale-sereno/releases/tag/v1.5.0' },
    notes: '### Mods adicionados\n- Sophisticated Backpacks 3.20.17',
    ...overrides,
  };
}
