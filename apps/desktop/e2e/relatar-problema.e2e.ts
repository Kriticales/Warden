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
 * - CA-UI02-05: o atalho Ctrl+Shift+E abre o diálogo de outra tela, e Esc fecha.
 *
 * O navegador não abre: com `WARDEN_E2E_OPEN_URL` (`wdio.conf.ts`), o app grava nesse arquivo
 * o endereço que abriria.
 */
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { hostname, userInfo } from 'node:os';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

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

  after(() => {
    // Devolve a pasta de dados como estava: o settings.e2e.ts, que roda depois (ordem
    // alfabética), começa pela primeira execução.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(FAKE_LOG, { force: true });
    rmSync(OPENED_FILE, { force: true });
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

  it('CA-UI02-02 e CA-UI02-04: a pré-visualização é o que o navegador recebe, sem dados pessoais', async () => {
    await $('[data-testid="report-title"]').setValue('E2E: o Testar fechou sozinho');
    await $('[data-testid="report-happened"]').setValue('A janela do jogo fechou na hora.');
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
    expect(params.get('labels')).toBe('relato-do-dono');
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
    await browser.keys(['Escape']);
    await dialog().waitForDisplayed({ reverse: true });
  });
});
