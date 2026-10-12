/**
 * "Relatar problema" no app real (UI-02):
 *
 * - CA-UI02-01: o botão (no Linux, no rodapé) abre o diálogo; sem título, "Abrir no GitHub"
 *   fica desabilitado; o diálogo passa no axe sem violação séria;
 * - CA-UI02-02: a pré-visualização mostra o que foi escrito e o que o Warden coletou, e o
 *   navegador recebe a página de issue nova do Warden com o título, o corpo exato da
 *   pré-visualização e o rótulo `relato-do-dono`;
 * - CA-UI02-04: um registro com a pasta do usuário, chaves nos formatos da CurseForge, do
 *   Gemini e do GitHub e o nome do computador não deixa nada disso no corpo nem no endereço;
 * - CA-UI02-05: o atalho Ctrl+Shift+E abre o diálogo de outra tela, e Esc fecha;
 * - foco: fechado, o foco volta ao botão (aberto por ele) ou ao que estava focado antes do
 *   atalho (sem nada focado, ao botão).
 *
 *
 * UI-03, com o GitHub simulado (`e2e/mock-server/github.ts`):
 *
 * - CA-UI03-02: "Entrar com o GitHub" mostra o código, abre só a página do dispositivo e
 *   conecta depois da aprovação; "Sair do GitHub" apaga o login;
 * - CA-UI03-05: com o login, o relato vai direto, com categoria e um print escolhido, e a
 *   tela mostra "Relato enviado (#número)" com o link;
 * - CA-UI03-08: uma falha mantém o rascunho e "Tentar de novo" envia;
 * - CA-UI03-09: nenhum token nos registros, no corpo da issue nem no cofre depois de sair.
 *
 * O navegador não abre: com `WARDEN_E2E_OPEN_URL` (`wdio.conf.ts`), o app grava nesse arquivo
 * o endereço que abriria. O diálogo de imagens lê os caminhos de `WARDEN_E2E_PICK_FILES`.
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { hostname, userInfo } from 'node:os';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import {
  E2E_ACCESS_TOKEN,
  E2E_DEVICE_CODE,
  E2E_LOGIN,
  E2E_REFRESH_TOKEN,
  E2E_USER_CODE,
  type GithubState,
} from './mock-server/github';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');
const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const OPENED_FILE = env('WARDEN_E2E_OPEN_URL');
const PICK_FILES = env('WARDEN_E2E_PICK_FILES');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const LOGIN_FILE = join(DATA_ROOT, 'config', 'github-login.json');
const VAULT_DIR = join(DATA_ROOT, 'cofre-de-teste');
/** Um PNG de 1×1 de verdade (o Rust confere o conteúdo). */
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);
const PRINT_FILE = join(DATA_ROOT, 'print-e2e.png');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const LOGS_DIR = join(DATA_ROOT, 'data', 'logs');
/** Um registro "do futuro": é o arquivo mais novo da pasta, lido primeiro pelo relato. */
const FAKE_LOG = join(LOGS_DIR, 'warden.9999-12-31.log');
const ISSUE_PREFIX = 'https://github.com/Kriticales/Warden/issues/new?';

/** Valores falsos nos formatos reais, montados em pedaços (nunca uma chave de verdade). */
const FAKE_SECRETS = [
  ['$2a$10$', 'e2eFalsaChaveDaCurseForge0123456789abcdefghijklm'].join(''),
  ['AIza', 'SyE2eFalsaChaveDoGemini0123456789abc'].join(''),
  ['ghp_', 'E2eFalsoTokenDoGitHub0123456789abcdef'].join(''),
];
const USER = userInfo().username;
const COMPUTER = hostname();

/** O que não pode aparecer: segredos e os nomes de 4 letras ou mais (os curtos só como palavra). */
function forbidden(): string[] {
  return [...FAKE_SECRETS, ...[USER, COMPUTER].filter((name) => name.length >= 4)];
}

function leaks(text: string): string[] {
  const lower = text.toLowerCase();
  return forbidden().filter((value) => lower.includes(value.toLowerCase()));
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(join(SCREENSHOTS, `relato-${name}.png`));
}

interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
}

async function seriousViolations(): Promise<string[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async () => {
    const { axe } = window as unknown as {
      axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
    };
    try {
      return (await axe.run({ resultTypes: ['violations'] })).violations;
    } catch {
      const failed: AxeViolation = {
        id: 'axe-falhou',
        impact: 'critical',
        help: 'o axe não rodou',
      };
      return [failed];
    }
  });
  return violations
    .filter((violation) => violation.impact === 'serious' || violation.impact === 'critical')
    .map((violation) => `${violation.id}: ${violation.help}`);
}

const dialog = () => $('[role="dialog"]');
const openButton = () => $('[data-testid="report-open"]');
const previewBody = () => $('[data-testid="report-preview-body"]');

/**
 * Marca o elemento focado agora (`data-e2e-focado`); devolve `false` se nada está focado.
 */
async function markFocused(): Promise<boolean> {
  return browser.execute(() => {
    const active = document.activeElement;
    if (!active || active === document.body) return false;
    active.setAttribute('data-e2e-focado', '');
    return true;
  });
}

/** Quem está focado: `marcado`, o `data-testid` ou o nome da tag (`BODY` se nada). */
async function focused(): Promise<string> {
  return browser.execute(() => {
    const active = document.activeElement;
    if (!active) return 'nenhum';
    if (active.hasAttribute('data-e2e-focado')) return 'marcado';
    return active.getAttribute('data-testid') ?? active.tagName;
  });
}

/** Fecha o diálogo com Esc e espera ele sumir. */
async function closeWithEscape(): Promise<void> {
  await browser.keys(['Escape']);
  await dialog().waitForDisplayed({ reverse: true });
}

/** Texto exato da pré-visualização do corpo (o `<pre>`). */
async function previewText(): Promise<string> {
  return browser.execute(
    () => document.querySelector('[data-testid="report-preview-body"]')?.textContent ?? '',
  );
}

describe('Relatar problema', () => {
  before(async () => {
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    mkdirSync(LOGS_DIR, { recursive: true });
    writeFileSync(
      FAKE_LOG,
      [
        `2026-10-12T10:00:00Z INFO warden: pasta C:\\Users\\${USER}\\AppData\\Roaming\\warden`,
        `2026-10-12T10:00:01Z INFO warden: pasta /home/${USER}/.local/share/warden`,
        `2026-10-12T10:00:02Z DEBUG warden: chave da CurseForge ${FAKE_SECRETS[0] ?? ''}`,
        `2026-10-12T10:00:03Z DEBUG warden: chave do Gemini ${FAKE_SECRETS[1] ?? ''}`,
        `2026-10-12T10:00:04Z DEBUG warden: token do GitHub ${FAKE_SECRETS[2] ?? ''}`,
        `2026-10-12T10:00:05Z INFO warden: computador ${COMPUTER}`,
        '2026-10-12T10:00:06Z INFO warden: última linha do registro de teste',
        '',
      ].join('\n'),
    );
    rmSync(OPENED_FILE, { force: true });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
    await $('h1=Meus packs').waitForDisplayed();
  });

  it('CA-UI02-01: o botão abre o diálogo; sem título não abre o GitHub; sem violação séria', async () => {
    const button = $('[data-testid="report-button"]');
    await button.waitForClickable();
    await expect(button).toHaveText('Relatar problema');
    await button.click();
    await dialog().waitForDisplayed();
    await expect($('h2=Relatar problema')).toBeDisplayed();
    await expect(
      $('div*=Este repositório é público: o relato fica visível para qualquer pessoa.'),
    ).toBeDisplayed();
    await previewBody().waitForDisplayed();
    await expect(openButton()).toBeDisabled();
    expect(await seriousViolations()).toEqual([]);
  });

  it('fechado com Esc ou Cancelar, o foco volta ao botão "Relatar problema"', async () => {
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'o foco não voltou ao botão depois do Esc',
    });
    const button = $('[data-testid="report-button"]');
    await button.click();
    await dialog().waitForDisplayed();
    await $('button=Cancelar').click();
    await dialog().waitForDisplayed({ reverse: true });
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'o foco não voltou ao botão depois do Cancelar',
    });
    // O próximo teste continua com o diálogo aberto.
    await button.click();
    await dialog().waitForDisplayed();
    await previewBody().waitForDisplayed();
  });

  it('CA-UI02-02 e CA-UI02-04: a pré-visualização é o que o navegador recebe, sem dados pessoais', async () => {
    await $('[data-testid="report-title"]').setValue('E2E: o Testar fechou sozinho');
    await $('[data-testid="report-description"]').setValue('A janela do jogo fechou na hora.');
    await browser.waitUntil(async () => (await previewText()).includes('fechou na hora'), {
      timeoutMsg: 'a pré-visualização não acompanhou o texto',
    });
    const collected = await $('[data-testid="report-collected"]').getText();
    expect(collected).toContain('Meus packs');
    expect(collected).toContain('Warden');
    const body = await previewText();
    expect(body).toContain('última linha do registro de teste');
    expect(body).toContain('<segredo>');
    expect(body).toContain('<usuário>');
    expect(leaks(body)).toEqual([]);
    await expect($('[data-testid="report-preview-title"]')).toHaveText(
      'E2E: o Testar fechou sozinho',
    );
    await screenshot('01-preenchido');

    await openButton().waitForEnabled();
    await openButton().click();
    await browser.waitUntil(() => existsSync(OPENED_FILE), {
      timeoutMsg: 'o app não pediu para abrir o navegador',
    });
    const url = readFileSync(OPENED_FILE, 'utf8');
    expect(url.startsWith(ISSUE_PREFIX)).toBe(true);
    expect(url.length).toBeLessThanOrEqual(7500);
    const params = new URL(url).searchParams;
    expect(params.get('title')).toBe('E2E: o Testar fechou sozinho');
    expect(params.get('labels')).toBe('relato-do-dono,categoria: erro');
    expect(params.get('body')).toBe(body);
    expect(leaks(url)).toEqual([]);
    expect(leaks(decodeURIComponent(url))).toEqual([]);

    await browser.waitUntil(
      async () =>
        browser.execute(() =>
          Array.from(document.querySelectorAll('.toast')).some((toast) =>
            toast.textContent.includes('Relato aberto no navegador'),
          ),
        ),
      { timeoutMsg: 'o aviso "Relato aberto no navegador" não apareceu' },
    );
    await dialog().waitForDisplayed({ reverse: true });
  });

  it('CA-UI02-05: o atalho Ctrl+Shift+E abre o diálogo em outra tela, e Esc fecha', async () => {
    const link = $('header.topbar').$('a*=Configurações');
    await link.waitForClickable();
    await link.click();
    await $('h1=Configurações').waitForDisplayed();
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    const collected = $('[data-testid="report-collected"]');
    await collected.waitForDisplayed();
    await expect(collected).toHaveText(expect.stringContaining('Configurações'));
    await screenshot('02-atalho');
    await closeWithEscape();
  });

  it('aberto pelo atalho, o foco volta ao que estava focado; sem nada focado, ao botão', async () => {
    const tasks = $('[data-testid="tasks-indicator"]');
    await browser.execute(() => {
      document.querySelector<HTMLElement>('[data-testid="tasks-indicator"]')?.focus();
    });
    expect(await markFocused()).toBe(true);
    await expect(tasks).toBeFocused();
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'marcado', {
      timeoutMsg: 'o foco não voltou ao elemento focado antes do atalho',
    });

    await browser.execute(() => {
      (document.activeElement as HTMLElement | null)?.blur();
    });
    expect(await focused()).toBe('BODY');
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'sem nada focado antes, o foco não foi para o botão',
    });
  });
});

/** Lê o estado do GitHub simulado (ou manda um controle: `approve`, `fail-next-issue`). */
async function github(control?: string, body?: object): Promise<GithubState> {
  const response = await fetch(`${MOCK_URL}/__github${control ? `/${control}` : ''}`, {
    method: control ? 'POST' : 'GET',
    body: body ? JSON.stringify(body) : null,
  });
  return (await response.json()) as GithubState;
}

/** Os tokens e o código secreto do GitHub simulado: nada disso pode vazar. */
const GITHUB_SECRETS = [E2E_ACCESS_TOKEN, E2E_REFRESH_TOKEN, E2E_DEVICE_CODE];

function filesUnder(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? filesUnder(join(dir, entry.name)) : [join(dir, entry.name)],
  );
}

const loginSection = () => $('[data-testid="github-login"]');

async function goToSettings(): Promise<void> {
  const link = $('header.topbar').$('a*=Configurações');
  await link.waitForClickable();
  await link.click();
  await $('h1=Configurações').waitForDisplayed();
  await loginSection().scrollIntoView();
  await loginSection().waitForDisplayed();
}

async function openReportByShortcut(): Promise<void> {
  await browser.keys(['Control', 'Shift', 'e']);
  await dialog().waitForDisplayed();
  await previewBody().waitForDisplayed();
}

const sendButton = () => $('[data-testid="report-send"]');

describe('Relatar problema direto no GitHub (UI-03)', () => {
  before(async () => {
    await github('reset');
    writeFileSync(PRINT_FILE, PNG);
    writeFileSync(PICK_FILES, `${PRINT_FILE}\n`);
    rmSync(OPENED_FILE, { force: true });
  });

  it('CA-UI03-02: Entrar com o GitHub mostra o código, abre só a página do dispositivo e conecta', async () => {
    await goToSettings();
    const section = loginSection();
    const enter = section.$('button=Entrar com o GitHub');
    await enter.waitForClickable();
    await enter.click();
    const code = $('[data-testid="github-login-code"]');
    await code.waitForDisplayed();
    await expect(code).toHaveText(E2E_USER_CODE);
    await expect(section.$('span*=Aguardando você aprovar no GitHub')).toBeDisplayed();
    expect(await seriousViolations()).toEqual([]);
    await screenshot('03-codigo-do-github');

    await section.$('button=Abrir o GitHub').click();
    await browser.waitUntil(() => existsSync(OPENED_FILE), {
      timeoutMsg: 'o app não pediu para abrir a página do GitHub',
    });
    expect(readFileSync(OPENED_FILE, 'utf8')).toBe(`${MOCK_URL}/github/site/login/device`);
    rmSync(OPENED_FILE, { force: true });

    // A pessoa aprova no GitHub; a próxima consulta (a cada 5 s) traz o token.
    await github('approve');
    await section.$(`span=Conectado como ${E2E_LOGIN}`).waitForDisplayed({ timeout: 30_000 });
    expect(existsSync(LOGIN_FILE)).toBe(true);
    const loginFile = readFileSync(LOGIN_FILE, 'utf8');
    for (const secret of GITHUB_SECRETS) expect(loginFile).not.toContain(secret);
  });

  it('CA-UI03-05: com o login, envia com categoria e print e mostra o número da issue', async () => {
    await openReportByShortcut();
    await $('[data-testid="report-category"]').selectByAttribute('value', 'suggestion');
    await $('[data-testid="report-title"]').setValue('E2E: um botão para copiar o console');
    await $('[data-testid="report-description"]').setValue('Seria bom copiar tudo de uma vez.');
    await $('[data-testid="report-choose-images"]').click();
    const thumb = $('img[alt="Print 1: print-e2e.png"]');
    await thumb.waitForDisplayed();
    await expect(
      $('div*=Os prints vão para o repositório do Warden e ficam visíveis como o relato.'),
    ).toBeDisplayed();
    await browser.waitUntil(
      async () => (await previewText()).includes('![Print 1: print-e2e.png]'),
      {
        timeoutMsg: 'a pré-visualização não mostrou o print',
      },
    );
    expect(await previewText()).toContain('**Categoria:** Sugestão');
    expect(await seriousViolations()).toEqual([]);
    await screenshot('04-relato-com-print');

    await sendButton().waitForEnabled();
    await sendButton().click();
    const sent = $('[data-testid="report-sent"]');
    await sent.waitForDisplayed({ timeout: 30_000 });
    await expect(sent).toHaveText(expect.stringContaining('Relato enviado (#101)'));

    const state = await github();
    expect(state.labels).toEqual(['relato-do-dono', 'categoria: sugestão']);
    expect(state.branch).toBe(true);
    expect(state.orphanCommits).toBe(1);
    expect(state.files).toHaveLength(1);
    expect(state.files[0]?.branch).toBe('relatos-anexos');
    expect(state.files[0]?.path).toMatch(/^anexos\/[0-9a-f]{20}\.png$/);
    expect(state.files[0]?.bytes).toBe(PNG.length);
    const issue = state.issues[0];
    expect(issue?.title).toBe('E2E: um botão para copiar o console');
    expect(issue?.labels).toEqual(['relato-do-dono', 'categoria: sugestão']);
    const body = issue?.body ?? '';
    expect(body.startsWith('**Categoria:** Sugestão')).toBe(true);
    expect(body).toContain('Seria bom copiar tudo de uma vez.');
    expect(body).toContain(
      `![Print 1: print-e2e.png](${MOCK_URL}/github/site/Kriticales/Warden/blob/relatos-anexos/${state.files[0]?.path ?? ''}?raw=true)`,
    );
    // CA-UI03-09: nada de token nem de dados pessoais no corpo.
    for (const secret of GITHUB_SECRETS) expect(body).not.toContain(secret);
    expect(leaks(body)).toEqual([]);

    await $('button=Ver no GitHub').click();
    await browser.waitUntil(() => existsSync(OPENED_FILE), {
      timeoutMsg: 'o app não pediu para abrir a issue',
    });
    expect(readFileSync(OPENED_FILE, 'utf8')).toBe(
      `${MOCK_URL}/github/site/Kriticales/Warden/issues/101`,
    );
    rmSync(OPENED_FILE, { force: true });
    await $('button=Concluir').click();
    await dialog().waitForDisplayed({ reverse: true });
  });

  it('CA-UI03-08: uma falha mantém o rascunho, e Tentar de novo envia', async () => {
    await github('fail-next-issue', { status: 403 });
    await openReportByShortcut();
    await $('[data-testid="report-title"]').setValue('E2E: limite da API');
    await sendButton().waitForEnabled();
    await sendButton().click();
    await $('div*=O GitHub limitou os pedidos do Warden por um tempo.').waitForDisplayed({
      timeout: 30_000,
    });
    await expect($('[data-testid="report-title"]')).toHaveValue('E2E: limite da API');
    await expect($('button=Abrir no GitHub')).toBeDisplayed();
    expect(await seriousViolations()).toEqual([]);
    await screenshot('05-falha-no-envio');
    await $('button=Tentar de novo').click();
    const sent = $('[data-testid="report-sent"]');
    await sent.waitForDisplayed({ timeout: 30_000 });
    await expect(sent).toHaveText(expect.stringContaining('Relato enviado (#102)'));
    expect((await github()).issues.map((issue) => issue.title)).toEqual([
      'E2E: um botão para copiar o console',
      'E2E: limite da API',
    ]);
    await $('button=Concluir').click();
    await dialog().waitForDisplayed({ reverse: true });
  });

  it('CA-UI03-09: nenhum token nos registros do Warden', () => {
    const files = filesUnder(LOGS_DIR);
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const text = readFileSync(file, 'utf8');
      for (const secret of GITHUB_SECRETS) {
        expect(text.includes(secret)).toBe(false);
      }
    }
  });

  it('CA-UI03-02: Sair do GitHub apaga o login e os dois tokens', async () => {
    await goToSettings();
    const section = loginSection();
    await section.$('button=Sair do GitHub').click();
    await section.$('button=Entrar com o GitHub').waitForDisplayed();
    expect(existsSync(LOGIN_FILE)).toBe(false);
    for (const file of filesUnder(VAULT_DIR)) {
      const text = readFileSync(file, 'utf8');
      for (const secret of GITHUB_SECRETS) expect(text.includes(secret)).toBe(false);
    }
  });
});

after(() => {
  // Devolve a pasta de dados como estava: o settings.e2e.ts, que roda depois (ordem
  // alfabética), começa pela primeira execução.
  rmSync(SETTINGS_FILE, { force: true });
  rmSync(FAKE_LOG, { force: true });
  rmSync(OPENED_FILE, { force: true });
  rmSync(PRINT_FILE, { force: true });
  rmSync(PICK_FILES, { force: true });
  rmSync(LOGIN_FILE, { force: true });
});
