/**
 * Console agrupado no app real (D-09; SPEC T13 "Mostrar: Linha a linha · Agrupado por mod · Só
 * problemas"), com o jogo simulado da `warden-launcher` (`WardenFakeGame`), como o
 * `test.e2e.ts`:
 *
 * - com o jogo aberto, "Agrupado por mod" junta o batimento repetido do jogo simulado num item
 *   "×N" do grupo "Desconhecido" (sem logger), e o número cresce enquanto o jogo escreve;
 * - numa sessão gravada (modo `acentos`), "Só problemas" mostra o aviso e esconde as linhas de
 *   informação; "Linha a linha" continua mostrando tudo.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { dismissToasts } from './toasts';

const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-console-agrupado');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

function sha256(data: string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 só com uma config e o modo do jogo simulado. */
function writePack(fakeGame: string): void {
  mkdirSync(join(PACK, 'config'), { recursive: true });
  const config = 'dificuldade = 2\n';
  writeFileSync(join(PACK, 'config', 'vale.toml'), config);
  const index = `hash-format = "sha256"\n\n[[files]]\nfile = "config/vale.toml"\nhash = "${sha256(config)}"\n`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Console Agrupado"',
      'author = "Warden"',
      'version = "1.0.0"',
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
  writeFileSync(join(PACK, '.warden-fake-game'), fakeGame);
}

function testButton() {
  return $('header.packhead .testbtn__main');
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

function theConsole() {
  return $('section[aria-label="Console do jogo"]');
}

async function show(mode: string): Promise<void> {
  await dismissToasts();
  const select = theConsole().$('select[id]');
  await select.waitForDisplayed();
  await select.selectByVisibleText(mode);
}

/** O número de "×N" do item do batimento, ou 0 se ainda não apareceu. */
async function heartbeatCount(): Promise<number> {
  return browser.execute(() => {
    for (const line of document.querySelectorAll('.cline')) {
      if (line.textContent.includes('pai vivo')) {
        const rep = line.querySelector('.cline__rep')?.textContent ?? '';
        return Number(rep.replace(/\D/g, '')) || 0;
      }
    }
    return 0;
  });
}

describe('Console agrupado (D-09; T13)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do console agrupado pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack('filhos 1');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('com o jogo aberto, as repetições viram um item ×N que cresce', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    // O arquivo do jogo simulado pode aparecer na higiene: abre sem limpar.
    const open = $('button=Abrir pack');
    const notNow = $('button=Agora não');
    await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
      timeout: 60_000,
      timeoutMsg: 'Abrir pack não ficou pronto',
    });
    if (await notNow.isExisting()) await notNow.click();
    else await open.click();
    await waitHeading('Mods');

    await testButton().click();
    await waitHeading('Jogo aberto');
    await browser.waitUntil(async () => (await theConsole().getText()).includes('pai vivo'), {
      timeout: 30_000,
      timeoutMsg: 'o console não mostrou o jogo simulado',
    });

    await show('Agrupado por mod');
    const region = $('[role="region"][aria-label="Linhas agrupadas por mod"]');
    await region.waitForDisplayed();
    const unknown = region.$('button[aria-label="Mostrar as linhas de Desconhecido"]');
    await unknown.waitForClickable({ timeout: 15_000 });
    await unknown.click();
    await browser.waitUntil(async () => (await heartbeatCount()) >= 2, {
      timeout: 15_000,
      timeoutMsg: 'o batimento não virou um item ×N',
    });
    const before = await heartbeatCount();
    // Com o jogo aberto, o agrupado é relido a cada segundo.
    await browser.waitUntil(async () => (await heartbeatCount()) > before, {
      timeout: 15_000,
      timeoutMsg: 'a contagem não cresceu com o jogo escrevendo',
    });

    await show('Só problemas');
    await expect(
      $('[role="region"][aria-label="Erros, avisos e stack traces do console"]'),
    ).toHaveText(expect.stringContaining('Nenhum erro, aviso ou stack trace até agora.'));

    await show('Linha a linha');
    await expect(theConsole().$('[role="log"]')).toBeDisplayed();

    await dismissToasts();
    await $('main').$('button=Parar jogo').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await dialog.$('button=Parar jogo').click();
    await waitHeading('Encerrado por você', 15_000);
  });

  it('numa sessão gravada, "Só problemas" mostra o aviso e esconde a informação', async () => {
    writePack('acentos');
    await dismissToasts();
    await $('main').$('button=Testar de novo').click();
    await waitHeading('O jogo fechou normalmente');
    await $('main').$('button=Ver console').click();
    await theConsole().waitForDisplayed();

    await show('Só problemas');
    const problems = $('[role="region"][aria-label="Erros, avisos e stack traces do console"]');
    await browser.waitUntil(
      async () => (await problems.getText()).includes('Configuração inválida: ç ã õ é'),
      { timeout: 15_000, timeoutMsg: 'o aviso não apareceu em Só problemas' },
    );
    expect(await problems.getText()).not.toContain('Ação, coração');

    await show('Agrupado por mod');
    const region = $('[role="region"][aria-label="Linhas agrupadas por mod"]');
    const unknown = region.$('button[aria-label="Mostrar as linhas de Desconhecido"]');
    await unknown.waitForClickable({ timeout: 15_000 });
    await unknown.click();
    await browser.waitUntil(async () => (await region.getText()).includes('Ação, coração'), {
      timeout: 15_000,
      timeoutMsg: 'a linha de informação não apareceu no agrupado',
    });

    await show('Linha a linha');
    await expect(theConsole()).toHaveText(expect.stringContaining('Ação, coração'));
  });
});
