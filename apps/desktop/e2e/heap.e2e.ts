/**
 * Memória do jogo no app real (L-11; SPEC T13; CA-T13-10, parte da faixa), com o jogo simulado
 * da `warden-launcher` no modo `marcadores`: ele é uma JVM de verdade, que publica o
 * `hsperfdata` (no Linux em `/tmp`, no Windows pelo *file mapping*), e o app lê a memória sem
 * JDK e sem mexer no jogo.
 *
 * - Com o jogo aberto, a faixa mostra "Memória do jogo" (usada de máxima, com o gráfico) e
 *   "Coletas de memória", sem violações sérias do axe.
 * - Com `-XX:+PerfDisableSharedMem` em Ajustes do teste, a faixa diz "sem leitura" e o motivo,
 *   o aviso abaixo dela abre Ajustes do teste, que oferece remover o argumento.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-memoria');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

function sha256(data: string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 vazio na versão dada, com o modo do jogo simulado. */
function writePack(version: string, fakeGame: string): void {
  mkdirSync(PACK, { recursive: true });
  const index = 'hash-format = "sha256"\n';
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale da Memória"',
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
  writeFileSync(join(PACK, '.warden-fake-game'), fakeGame);
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function testButton() {
  return $('header.packhead .testbtn__main');
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

const LIVE_STRIP = 'section[aria-label="Desempenho do jogo"]';

/** O texto visível de uma parte da tela (pelo `innerText`, como no `perf.e2e.ts`). */
async function textOf(selector: string): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector<HTMLElement>(css)?.innerText ?? '',
    selector,
  );
}

/** Espera o texto da faixa casar com tudo o que foi pedido. */
async function waitStripText(parts: readonly (string | RegExp)[]): Promise<string> {
  let text = '';
  const matches = (part: string | RegExp) =>
    typeof part === 'string' ? text.includes(part) : part.test(text);
  try {
    await browser.waitUntil(
      async () => {
        text = await textOf(LIVE_STRIP);
        return parts.every(matches);
      },
      { timeout: 30_000 },
    );
  } catch {
    throw new Error(`a faixa não mostrou ${parts.join(', ')}: ${JSON.stringify(text)}`);
  }
  return text;
}

/** Para o jogo pelo botão (com a confirmação) e espera o resultado. */
async function stopGame(): Promise<void> {
  await dismissToasts();
  await $('main').$('button=Parar jogo').click();
  const dialog = $('[role="alertdialog"]');
  await dialog.waitForDisplayed();
  await dialog.$('button=Parar jogo').click();
  await waitHeading('Encerrado por você', 15_000);
}

/** O campo "Argumentos extras da JVM" do diálogo aberto. */
async function argsField() {
  const id = await browser.execute(
    () =>
      [...document.querySelectorAll('[role="dialog"] label')]
        .find((label) => label.textContent.includes('Argumentos extras da JVM'))
        ?.getAttribute('for') ?? '',
  );
  if (!id) throw new Error('campo dos argumentos da JVM');
  return $(`[id="${id}"]`);
}

/** Salva Ajustes do teste e espera o diálogo fechar. */
async function saveSettings(): Promise<void> {
  const dialog = $('[role="dialog"]');
  await dialog.$('button=Salvar ajustes').click();
  await dialog.waitForExist({ reverse: true, timeout: 15_000 });
}

describe('Memória do jogo pelo hsperfdata (L-11; CA-T13-10)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E da memória do jogo pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack('1.0.0', 'marcadores 300');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // A instância de teste deste pack sai junto: os outros E2E contam as instâncias.
    try {
      const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
        packs?: { id: string }[];
      };
      for (const pack of registry.packs ?? []) {
        rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
      }
    } catch {
      // Sem registro: nada foi aberto.
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('a faixa mostra a memória do jogo, o gráfico e as coletas com o jogo aberto', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
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

    await testButton().click();
    await waitHeading('Jogo aberto');
    await $(LIVE_STRIP).waitForExist({ timeout: 30_000 });
    // O jogo simulado ocupa 64 MB depois de entrar no mundo: a memória passa disso.
    const text = await waitStripText([
      /Memória do jogo\n(\d{2,4} MB|\d+,\d GB) de \d+(,\d)? GB/,
      /Coletas de memória\n\d+\n\d+,\d s parado em coletas/,
    ]);
    expect(text).not.toContain('sem leitura');
    await expect(
      $(`${LIVE_STRIP} [role="img"][aria-label^="Memória nos últimos 5 minutos"]`),
    ).toExist();
    await screenshot('52-faixa-memoria-do-jogo');
    expect(await seriousViolations()).toEqual([]);
    await stopGame();
  });

  it('com -XX:+PerfDisableSharedMem: "sem leitura", o motivo e Ajustes do teste remove o argumento', async () => {
    await dismissToasts();
    await $('header.packhead .testbtn__more').click();
    await $('[role="menuitem"]*=Ajustes do teste neste computador').click();
    const field = await argsField();
    await field.waitForDisplayed({ timeout: 15_000 });
    await field.setValue('-XX:+PerfDisableSharedMem');
    await saveSettings();

    await dismissToasts();
    await $('main').$('button=Testar de novo').click();
    await waitHeading('Jogo aberto');
    await waitStripText([
      'Memória do jogo\nsem leitura\nOs argumentos do teste têm -XX:+PerfDisableSharedMem.',
      'Coletas de memória\n—\nsem leitura',
    ]);
    const alert = $('.alert*=A memória do jogo não pode ser lida neste teste.');
    await alert.waitForDisplayed({ timeout: 15_000 });
    await expect(alert).toHaveText(expect.stringContaining('comum nas “flags do Aikar”'));
    await screenshot('53-faixa-sem-leitura');
    expect(await seriousViolations()).toEqual([]);

    await dismissToasts();
    await alert.$('button=Ajustes do teste…').click();
    const dialog = $('[role="dialog"]');
    await dialog.$('button=Remover estes argumentos').waitForDisplayed({ timeout: 15_000 });
    await dialog.$('button=Remover estes argumentos').click();
    await browser.waitUntil(async () => (await (await argsField()).getValue()) === '', {
      timeout: 5_000,
      timeoutMsg: 'o argumento não saiu do campo',
    });
    await saveSettings();
    await stopGame();
  });
});
