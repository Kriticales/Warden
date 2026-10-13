/**
 * Travamentos em Problemas no app real (D-06; SPEC T14; CA-T14-08), com o jogo simulado da
 * `warden-launcher` no modo `crash-npe`: a cada abertura ele grava um crash report "Ticking
 * entity" com NullPointerException no mesmo mod, mas com números diferentes (linhas da pilha,
 * índice do jar, coordenadas, horário e nome do arquivo), e trava.
 *
 * - O pack trava na versão 1.0.0 e de novo na 1.1.0. Os dois crash reports têm números
 *   diferentes; em Problemas → Travamentos aparece **uma** linha "Travou 2 vezes", com as duas
 *   versões, a situação e "Ver as 2 vezes" com cada teste.
 * - "Ver o que causou" abre o resultado do teste mais novo.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-travamentos');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

const CRASHES = 'section[aria-labelledby="diag-crashes-title"]';
const SESSIONS_ROWS = 'section[aria-labelledby="test-sessions-title"] tbody tr';

function sha256(data: string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 vazio na versão dada, com o jogo simulado que trava. */
function writePack(version: string): void {
  mkdirSync(PACK, { recursive: true });
  const index = 'hash-format = "sha256"\n';
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale Instável"',
      'author = "Warden"',
      `version = "${version}"`,
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      `hash = "${sha256(index)}"`,
      '',
      '[versions]',
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, '.warden-fake-game'), 'crash-npe');
}

/** Os ids dos packs registrados (para achar e limpar a instância de teste). */
function registeredPacks(): { id: string; path: string }[] {
  if (!existsSync(REGISTRY_FILE)) return [];
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs?: { id: string; path: string }[];
  };
  return registry.packs ?? [];
}

/** Os crash reports que o jogo simulado gravou na instância deste pack. */
function crashReports(): string[] {
  const pack = registeredPacks().find((entry) => entry.path === PACK);
  if (!pack) return [];
  const dir = join(DATA_ROOT, 'data', 'instances', pack.id, 'minecraft', 'crash-reports');
  if (!existsSync(dir)) return [];
  return readdirSync(dir).map((name) => readFileSync(join(dir, name), 'utf8'));
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

/** Espera os testes anteriores do resultado terem `count` linhas (o teste novo foi gravado). */
async function waitSessions(count: number): Promise<void> {
  await browser.waitUntil(async () => (await $$(SESSIONS_ROWS).length) === count, {
    timeout: 90_000,
    timeoutMsg: `os testes anteriores não chegaram a ${String(count)}`,
  });
}

/** Clica depois de fechar os avisos e de o alvo parar de mexer. */
async function clickStable(element: ReturnType<typeof $>): Promise<void> {
  await dismissToasts();
  await element.waitForClickable({ timeout: 30_000 });
  await element.waitForStable({ timeout: 10_000 });
  await element.click();
}

describe('Travamentos em Problemas (D-06; CA-T14-08)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E dos travamentos pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack('1.0.0');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // A instância de teste deste pack sai junto: os outros E2E contam as instâncias.
    for (const pack of registeredPacks()) {
      rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('dois travamentos com números diferentes viram uma linha "Travou 2 vezes" com as duas versões', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    writeFileSync(PICK_FOLDER, PACK);
    await $('button=Abrir ou importar…').click();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    const open = $('button=Abrir pack');
    const notNow = $('button=Agora não');
    await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
      timeout: 60_000,
      timeoutMsg: 'Abrir pack não ficou pronto',
    });
    if (await notNow.isExisting()) await notNow.click();
    else await open.click();
    await waitHeading('Mods');

    // Primeira vez, na 1.0.0.
    await clickStable($('header.packhead .testbtn__main'));
    await waitHeading('O jogo travou', 90_000);
    await waitSessions(1);

    // Segunda vez, na 1.1.0.
    writePack('1.1.0');
    await clickStable($('main').$('button=Testar de novo'));
    await waitSessions(2);
    await waitHeading('O jogo travou', 90_000);

    // Os dois crash reports têm números diferentes.
    const reports = crashReports();
    expect(reports).toHaveLength(2);
    const lines = reports.map((text) => /EntityTicker\.java:(\d+)/.exec(text)?.[1]);
    expect(lines[0]).toBeDefined();
    expect(lines[0]).not.toBe(lines[1]);
    const places = reports.map((text) => /Exact location: (.+)/.exec(text)?.[1]);
    expect(places[0]).not.toBe(places[1]);

    await clickStable(sectionLink('Problemas'));
    await waitHeading('Problemas');
    const block = $(CRASHES);
    await block.waitForDisplayed({ timeout: 60_000 });
    const row = block.$('tr*=Travou 2 vezes');
    await row.waitForDisplayed({ timeout: 60_000 });
    // Uma linha só, com as versões das duas vezes e a causa encontrada.
    expect(await block.$$('tbody tr').length).toBe(1);
    await expect(row).toHaveText(
      expect.stringContaining('Uma criatura ou objeto do mundo quebrou o jogo (meumod:golem)'),
    );
    await expect(row).toHaveText(expect.stringContaining('versões 1.0.0 e 1.1.0'));
    await expect(row).toHaveText(expect.stringContaining('Causa encontrada'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d06-01-travamentos');

    // Cada vez que travou.
    await clickStable(row.$('button=Ver as 2 vezes'));
    await browser.waitUntil(
      async () => (await block.$$('ul.diag-crashes__occurrences li').length) === 2,
      {
        timeout: 30_000,
        timeoutMsg: 'as duas vezes não apareceram',
      },
    );
    await expect(block.$('ul.diag-crashes__occurrences')).toHaveText(
      expect.stringContaining('1.1.0'),
    );
    await expect(block.$('ul.diag-crashes__occurrences')).toHaveText(
      expect.stringContaining('1.0.0'),
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d06-02-cada-vez');
  });

  it('"Ver o que causou" abre o resultado do teste mais novo', async () => {
    const row = $(CRASHES).$('tr*=Travou 2 vezes');
    await clickStable(row.$('a=Ver o que causou'));
    await waitHeading('O jogo travou');
    const why = $('section[aria-labelledby="why-crashed-title"]');
    await why
      .$('article.issue*=Uma criatura ou objeto do mundo quebrou o jogo')
      .waitForDisplayed({ timeout: 30_000 });
    // O teste aberto (a linha marcada em "Testes anteriores") é o mais novo, o da 1.1.0.
    const current = $('section[aria-labelledby="test-sessions-title"] tr.is-selected');
    await current.waitForDisplayed({ timeout: 30_000 });
    await expect(current).toHaveText(expect.stringContaining('1.1.0'));
    await screenshot('d06-03-ver-o-que-causou');
  });
});
