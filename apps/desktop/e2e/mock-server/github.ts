/**
 * O GitHub simulado dos E2E da publicação (V-03): a API REST que o app usa, o servidor git
 * "smart HTTP" e o `raw.githubusercontent.com`, todos em `/github/…` no servidor de fixtures.
 *
 * - `/github/api/…`: `GET /user`, `POST /user/repos`, `GET /repos/{dono}/{repo}` e as Releases;
 * - `/github/git/<dono>/<repo>.git/…`: `info/refs?service=git-receive-pack` e
 *   `git-receive-pack`, atendidos pelo `git receive-pack --stateless-rpc` do git instalado
 *   (o mesmo programa que o GitHub e o `git http-backend` usam) sobre um repositório nu numa
 *   pasta temporária;
 * - `/github/raw/<dono>/<repo>/main/<caminho>`: o arquivo da `main` desse repositório.
 *
 * O app só usa o git embutido; o git instalado aqui é só o lado do servidor. Os testes leem o
 * estado (repositórios e Releases) por `GET /__github`. Pedidos sem o token esperado recebem 401.
 */
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** O token que o E2E salva em Configurações (de mentira, com o formato de um token clássico). */
export const GITHUB_TOKEN = 'ghp_tokenDoE2eQueNuncaVaiParaLugarNenhum01';
export const GITHUB_LOGIN = 'dono-e2e';

interface Release {
  id: number;
  tag: string;
  name: string;
  body: string;
}

interface Repo {
  dir: string;
  private: boolean;
}

export interface GithubState {
  repos: Record<string, { private: boolean; dir: string }>;
  releases: Release[];
}

const repos = new Map<string, Repo>();
const releases: Release[] = [];
let root: string | null = null;

function reposRoot(): string {
  root ??= mkdtempSync(join(tmpdir(), 'warden-e2e-github-'));
  return root;
}

export function githubState(): GithubState {
  return {
    repos: Object.fromEntries([...repos].map(([name, repo]) => [name, { ...repo }])),
    releases: [...releases],
  };
}

function readBody(request: IncomingMessage): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    request.on('data', (chunk: Buffer) => chunks.push(chunk));
    request.on('end', () => {
      resolve(Buffer.concat(chunks));
    });
    request.on('error', reject);
  });
}

function json(response: ServerResponse, status: number, body: unknown): void {
  response
    .writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
    .end(JSON.stringify(body));
}

function repoJson(base: string, full: string, repo: Repo) {
  const [owner, name] = full.split('/');
  return { name, owner: { login: owner }, private: repo.private, html_url: `${base}/${full}` };
}

function authorized(request: IncomingMessage, scheme: 'Bearer' | 'Basic'): boolean {
  const expected =
    scheme === 'Bearer'
      ? `Bearer ${GITHUB_TOKEN}`
      : `Basic ${Buffer.from(`x-access-token:${GITHUB_TOKEN}`).toString('base64')}`;
  return request.headers.authorization === expected;
}

async function serveApi(
  request: IncomingMessage,
  response: ServerResponse,
  route: string,
  base: string,
): Promise<void> {
  if (!authorized(request, 'Bearer')) {
    json(response, 401, { message: 'Bad credentials' });
    return;
  }
  const post = request.method === 'POST';
  const parts = route.split('/');
  if (!post && route === 'user') {
    json(response, 200, { login: GITHUB_LOGIN });
    return;
  }
  if (post && route === 'user/repos') {
    const body = JSON.parse((await readBody(request)).toString('utf8')) as {
      name: string;
      private: boolean;
    };
    const full = `${GITHUB_LOGIN}/${body.name}`;
    if (repos.has(full)) {
      json(response, 422, { message: 'name already exists on this account' });
      return;
    }
    const dir = join(reposRoot(), `${GITHUB_LOGIN}__${body.name}.git`);
    execFileSync('git', ['init', '--bare', '--quiet', '--initial-branch=main', dir]);
    const repo = { dir, private: body.private };
    repos.set(full, repo);
    json(response, 201, repoJson(base, full, repo));
    return;
  }
  if (parts[0] === 'repos' && parts.length >= 3) {
    const full = `${parts[1] ?? ''}/${parts[2] ?? ''}`;
    const repo = repos.get(full);
    if (!repo) {
      json(response, 404, { message: 'Not Found' });
      return;
    }
    if (!post && parts.length === 3) {
      json(response, 200, repoJson(base, full, repo));
      return;
    }
    if (!post && parts[3] === 'releases' && parts[4] === 'tags') {
      const found = releases.find((release) => release.tag === parts[5]);
      if (found) {
        json(response, 200, {
          id: found.id,
          html_url: `https://github.com/${full}/releases/${found.tag}`,
        });
      } else {
        json(response, 404, { message: 'Not Found' });
      }
      return;
    }
    if (post && parts[3] === 'releases' && parts.length === 4) {
      const body = JSON.parse((await readBody(request)).toString('utf8')) as {
        tag_name: string;
        name: string;
        body: string;
      };
      const release = {
        id: 1000 + releases.length,
        tag: body.tag_name,
        name: body.name,
        body: body.body,
      };
      releases.push(release);
      json(response, 201, {
        id: release.id,
        html_url: `https://github.com/${full}/releases/${release.tag}`,
      });
      return;
    }
  }
  json(response, 404, { message: 'Not Found' });
}

function pkt(text: string): Buffer {
  return Buffer.from(`${(text.length + 4).toString(16).padStart(4, '0')}${text}`);
}

/**
 * `git receive-pack --stateless-rpc` com a entrada dada (`null`: sem entrada, como no
 * `--advertise-refs`, que não lê nada e sai logo); devolve a saída inteira.
 */
function receivePack(dir: string, args: string[], input: Buffer | null): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const child = spawn('git', ['receive-pack', '--stateless-rpc', ...args, dir], {
      stdio: [input === null ? 'ignore' : 'pipe', 'pipe', 'inherit'],
    });
    const out: Buffer[] = [];
    child.stdout?.on('data', (chunk: Buffer) => out.push(chunk));
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) resolve(Buffer.concat(out));
      else reject(new Error(`git receive-pack saiu com ${String(code)}`));
    });
    if (input !== null && child.stdin) {
      // Se o git sair antes de ler tudo, o erro vem pelo código de saída, não por um EPIPE
      // que derrubaria o servidor de fixtures.
      child.stdin.on('error', () => undefined);
      child.stdin.end(input);
    }
  });
}

async function serveGit(
  request: IncomingMessage,
  response: ServerResponse,
  route: string,
): Promise<void> {
  const match = /^([^/]+\/[^/]+)\.git\/(.*)$/.exec(route);
  const repo = match ? repos.get(match[1] ?? '') : undefined;
  if (!match || !repo) {
    response.writeHead(404).end();
    return;
  }
  if (!authorized(request, 'Basic')) {
    response.writeHead(401, { 'www-authenticate': 'Basic realm="GitHub"' }).end();
    return;
  }
  const action = match[2] ?? '';
  if (request.method === 'GET' && action === 'info/refs?service=git-receive-pack') {
    const refs = await receivePack(repo.dir, ['--advertise-refs'], null);
    response
      .writeHead(200, { 'content-type': 'application/x-git-receive-pack-advertisement' })
      .end(Buffer.concat([pkt('# service=git-receive-pack\n'), Buffer.from('0000'), refs]));
    return;
  }
  if (request.method === 'POST' && action === 'git-receive-pack') {
    const result = await receivePack(repo.dir, [], await readBody(request));
    response
      .writeHead(200, { 'content-type': 'application/x-git-receive-pack-result' })
      .end(result);
    return;
  }
  response.writeHead(404).end();
}

function serveRaw(response: ServerResponse, route: string): void {
  const match = /^([^/]+\/[^/]+)\/([^/]+)\/(.+)$/.exec(route);
  const repo = match ? repos.get(match[1] ?? '') : undefined;
  if (!match || !repo) {
    response.writeHead(404).end();
    return;
  }
  try {
    const content = execFileSync('git', [
      '-C',
      repo.dir,
      'show',
      `${match[2] ?? ''}:${decodeURIComponent(match[3] ?? '')}`,
    ]);
    response.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' }).end(content);
  } catch {
    response.writeHead(404).end();
  }
}

/** Atende `/github/…`; `base` é o endereço do servidor de fixtures. */
export function serveGithub(
  request: IncomingMessage,
  response: ServerResponse,
  path: string,
  base: string,
): void {
  const route = path.slice('/github/'.length);
  const done = (error: unknown) => {
    response.writeHead(500).end(String(error));
  };
  if (route.startsWith('api/')) {
    serveApi(request, response, route.slice('api/'.length), `${base}/github/git`).catch(done);
  } else if (route.startsWith('git/')) {
    serveGit(request, response, route.slice('git/'.length)).catch(done);
  } else if (route.startsWith('raw/')) {
    serveRaw(response, route.slice('raw/'.length));
  } else {
    response.writeHead(404).end();
  }
}
