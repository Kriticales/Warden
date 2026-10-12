/**
 * O GitHub simulado dos E2E (UI-03): o login pelo device flow e o envio do relato.
 *
 * O app aponta para este servidor com `WARDEN_LOGIN_BASE_GITHUB=<url>/github/site/`,
 * `WARDEN_API_BASE_GITHUB=<url>/github/api/` e `WARDEN_GITHUB_CLIENT_ID` (`wdio.conf.ts`).
 *
 * - O device flow responde `authorization_pending` até o teste aprovar
 *   (`POST /__github/approve`); depois, devolve os tokens falsos abaixo.
 * - A API só aceita o token de acesso falso (`Authorization: Bearer …`); qualquer outro: 401.
 * - O estado (labels, branch dos prints, arquivos e issues) é lido pelo teste em
 *   `GET /__github`; `POST /__github/fail-next-issue` faz a próxima criação de issue falhar
 *   com o código pedido; `POST /__github/reset` volta ao começo.
 *
 * Os tokens são montados em pedaços, nos formatos do GitHub, e nunca são de verdade.
 */

export const GITHUB_CLIENT_ID = 'Iv1.e2e-cliente-do-warden';
export const E2E_ACCESS_TOKEN = ['ghu_', 'E2eTokenDeAcessoFalso0123456789abcdef'].join('');
export const E2E_REFRESH_TOKEN = [
  'ghr_',
  'E2eTokenDeRenovacaoFalso0123456789abcdefghijklmnopqrstuv',
].join('');
export const E2E_DEVICE_CODE = 'e2e-codigo-do-dispositivo-secreto-0123456789abcdef';
export const E2E_USER_CODE = 'WDJB-MJHT';
export const E2E_LOGIN = 'DonoDoWarden';

const REPO = 'api/repos/Kriticales/Warden/';

export interface GithubIssue {
  number: number;
  title: string;
  body: string;
  labels: string[];
}

export interface GithubState {
  approved: boolean;
  labels: string[];
  branch: boolean;
  /** Commits criados sem pai (a branch órfã dos prints). */
  orphanCommits: number;
  files: { path: string; branch: string; bytes: number }[];
  issues: GithubIssue[];
  /** Status da próxima criação de issue (0 = normal). */
  failNextIssue: number;
}

function initial(): GithubState {
  return {
    approved: false,
    labels: ['relato-do-dono'],
    branch: false,
    orphanCommits: 0,
    files: [],
    issues: [],
    failNextIssue: 0,
  };
}

export const githubState: { current: GithubState } = { current: initial() };

export function resetGithub(): void {
  githubState.current = initial();
}

export interface GithubReply {
  status: number;
  body: unknown;
  headers?: Record<string, string>;
}

function tokens(): unknown {
  return {
    access_token: E2E_ACCESS_TOKEN,
    expires_in: 28_800,
    refresh_token: E2E_REFRESH_TOKEN,
    refresh_token_expires_in: 15_897_600,
    token_type: 'bearer',
    scope: '',
  };
}

function form(body: string): URLSearchParams {
  return new URLSearchParams(body);
}

/** Texto de um campo do JSON recebido (`''` se não for texto). */
function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function json(body: string): Record<string, unknown> {
  try {
    return JSON.parse(body || '{}') as Record<string, unknown>;
  } catch {
    return {};
  }
}

/**
 * A resposta do GitHub simulado para `route` (o caminho depois de `/github/`), ou `null` se o
 * caminho não for dele.
 */
export function githubReply(
  method: string,
  route: string,
  authorization: string | undefined,
  body: string,
  base: string,
): GithubReply | null {
  const state = githubState.current;
  const path = decodeURIComponent(route.split('?')[0] ?? '');
  if (method === 'POST' && path === 'site/login/device/code') {
    if (form(body).get('client_id') !== GITHUB_CLIENT_ID) {
      return { status: 200, body: { error: 'incorrect_client_credentials' } };
    }
    return {
      status: 200,
      body: {
        device_code: E2E_DEVICE_CODE,
        user_code: E2E_USER_CODE,
        verification_uri: `${base}/github/site/login/device`,
        expires_in: 900,
        interval: 5,
      },
    };
  }
  if (method === 'POST' && path === 'site/login/oauth/access_token') {
    const fields = form(body);
    if (fields.get('grant_type') === 'refresh_token') {
      return fields.get('refresh_token') === E2E_REFRESH_TOKEN
        ? { status: 200, body: tokens() }
        : { status: 200, body: { error: 'bad_refresh_token' } };
    }
    if (fields.get('device_code') !== E2E_DEVICE_CODE) {
      return { status: 200, body: { error: 'incorrect_device_code' } };
    }
    return state.approved
      ? { status: 200, body: tokens() }
      : { status: 200, body: { error: 'authorization_pending' } };
  }
  if (!path.startsWith('api/')) return null;
  if (authorization !== `Bearer ${E2E_ACCESS_TOKEN}`) {
    return { status: 401, body: { message: 'Bad credentials' } };
  }
  if (method === 'GET' && path === 'api/user') {
    return { status: 200, body: { login: E2E_LOGIN, id: 1 } };
  }
  if (!path.startsWith(REPO)) return { status: 404, body: { message: 'Not Found' } };
  const rest = path.slice(REPO.length);
  if (method === 'GET' && rest.startsWith('labels/')) {
    const name = rest.slice('labels/'.length);
    return state.labels.includes(name)
      ? { status: 200, body: { name } }
      : { status: 404, body: { message: 'Not Found' } };
  }
  if (method === 'POST' && rest === 'labels') {
    const name = text(json(body).name);
    if (state.labels.includes(name)) {
      return { status: 422, body: { message: 'Validation Failed' } };
    }
    state.labels.push(name);
    return { status: 201, body: { name } };
  }
  if (method === 'GET' && rest === 'git/ref/heads/relatos-anexos') {
    return state.branch
      ? { status: 200, body: { ref: 'refs/heads/relatos-anexos', object: { sha: 'c0ffee' } } }
      : { status: 404, body: { message: 'Not Found' } };
  }
  if (method === 'POST' && rest === 'git/trees') {
    return { status: 201, body: { sha: 'arvore-dos-anexos' } };
  }
  if (method === 'POST' && rest === 'git/commits') {
    const parents = json(body).parents;
    if (Array.isArray(parents) && parents.length === 0) state.orphanCommits += 1;
    return { status: 201, body: { sha: 'c0ffee' } };
  }
  if (method === 'POST' && rest === 'git/refs') {
    if (state.branch) return { status: 422, body: { message: 'Reference already exists' } };
    state.branch = json(body).ref === 'refs/heads/relatos-anexos';
    return { status: 201, body: { ref: 'refs/heads/relatos-anexos' } };
  }
  if (method === 'PUT' && rest.startsWith('contents/')) {
    const fields = json(body);
    const filePath = rest.slice('contents/'.length);
    if (state.files.some((file) => file.path === filePath)) {
      return { status: 422, body: { message: '"sha" wasn\'t supplied.' } };
    }
    state.files.push({
      path: filePath,
      branch: text(fields.branch),
      bytes: Buffer.from(text(fields.content), 'base64').length,
    });
    return { status: 201, body: { content: { path: filePath } } };
  }
  if (method === 'POST' && rest === 'issues') {
    if (state.failNextIssue !== 0) {
      const status = state.failNextIssue;
      state.failNextIssue = 0;
      return {
        status,
        body: { message: 'API rate limit exceeded' },
        headers: status === 403 ? { 'x-ratelimit-remaining': '0', 'retry-after': '1' } : {},
      };
    }
    const fields = json(body);
    const issue: GithubIssue = {
      number: 100 + state.issues.length + 1,
      title: text(fields.title),
      body: text(fields.body),
      labels: Array.isArray(fields.labels) ? fields.labels.map(String) : [],
    };
    state.issues.push(issue);
    return {
      status: 201,
      body: {
        number: issue.number,
        html_url: `https://github.com/Kriticales/Warden/issues/${String(issue.number)}`,
      },
    };
  }
  return { status: 404, body: { message: 'Not Found' } };
}
