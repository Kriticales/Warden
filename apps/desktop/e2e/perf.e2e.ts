/**
 * Desempenho em todo teste no app real (L-10; SPEC T13; CA-T13-09), com o jogo simulado da
 * `warden-launcher` no modo `marcadores`: ele escreve os marcadores de "carregou" e de "entrou
 * no mundo" do Minecraft 1.13+ depois de um tempo, ocupa 64 MB e fica aberto. O app mede a RAM
 * do processo de verdade.
 *
 * - Com o jogo aberto, a faixa acima do console mostra a RAM do processo, "Abriu em" e
 *   "Entrou no mundo em", sem violações sérias do axe.
 * - No resultado, "Abriu em" com o tempo; testando outra versão do pack, mais lenta para
 *   carregar, a diferença para a anterior ("… a mais que na 1.0.0"), ao vivo e no resultado.
 * - Em Problemas, "Desempenho do último teste" (bug-01) com a variação em % e "✦ Perguntar ao
 *   Sculk por que ficou mais lento".
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { openOrImport } from './open-or-import';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-desempenho');
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
      'name = "Vale Medido"',
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
const RESULT_BLOCK = 'section[aria-label="Desempenho do teste"]';

/**
 * O texto visível de uma parte da tela. Pelo `innerText` da página: o `getText` do driver do
 * WebKitGTK devolve vazio para a faixa (grade com `overflow: hidden`), embora ela apareça.
 */
async function textOf(selector: string): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector<HTMLElement>(css)?.innerText ?? '',
    selector,
  );
}

/**
 * Espera o texto da faixa casar com tudo o que foi pedido e não ter nada do que não pode mais
 * aparecer. Na falha, a mensagem traz o último texto lido.
 */
async function waitStripText(
  parts: readonly (string | RegExp)[],
  absent: readonly string[] = ['medindo…', 'carregando…', 'ainda não entrou num mundo'],
): Promise<string> {
  let text = '';
  const matches = (part: string | RegExp) =>
    typeof part === 'string' ? text.includes(part) : part.test(text);
  try {
    await browser.waitUntil(
      async () => {
        text = await textOf(LIVE_STRIP);
        return parts.every(matches) && !absent.some((part) => text.includes(part));
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
  await $('main').$('button=Parar jogo').click();
  const dialog = $('[role="alertdialog"]');
  await dialog.waitForDisplayed();
  await dialog.$('button=Parar jogo').click();
  await waitHeading('Encerrado por você', 15_000);
}

describe('Desempenho em todo teste (L-10; CA-T13-09)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do desempenho pulado');
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

  it('a faixa mostra a RAM do processo, "Abriu em" e o mundo com o jogo aberto', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await openOrImport();
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
    // RAM medida (com o pico), carregou e entrou no mundo. O tempo conta desde a abertura do
    // processo, com a partida da JVM: menos de 1 s numa máquina rápida, alguns segundos na CI.
    const text = await waitStripText(['pico de', /Abriu em\n(menos de 1 s|\d+ s)/]);
    expect(text).toMatch(/RAM do processo\n\d+(,\d)? (MB|GB)\npico de/);
    expect(text).toMatch(/Entrou no mundo em\n(menos de 1 s|\d+ s)/);
    await screenshot('49-faixa-de-desempenho');
    expect(await seriousViolations()).toEqual([]);

    await stopGame();
    await $(RESULT_BLOCK).waitForExist();
    const block = await textOf(RESULT_BLOCK);
    expect(block).toMatch(/Abriu em\n(menos de 1 s|\d+ s)/);
    expect(block).toMatch(/Pico de RAM do processo\n\d+(,\d)? (MB|GB)/);
    expect(await seriousViolations()).toEqual([]);
  });

  it('outra versão do pack, mais lenta: a diferença aparece ao vivo e no resultado', async () => {
    // Bem mais lenta: a partida da JVM varia alguns segundos entre máquinas (na CI, a primeira
    // abertura levou ~3 s mesmo pedindo 0,3 s).
    writePack('1.1.0', 'marcadores 8000');
    await $('main').$('button=Testar de novo').click();
    await waitHeading('Jogo aberto');
    const text = await waitStripText(['a mais que na 1.0.0']);
    expect(text).toMatch(/Abriu em\n\d+ s\n\d+ s a mais que na 1\.0\.0/);
    await screenshot('50-faixa-com-comparacao');
    await stopGame();
    await $(RESULT_BLOCK).waitForExist();
    expect(await textOf(RESULT_BLOCK)).toContain('a mais que na 1.0.0');
    await screenshot('51-resultado-abriu-em');
  });

  it('Problemas mostra o desempenho do último teste, com a variação e "✦ Perguntar ao Sculk"', async () => {
    await $('nav[aria-label="Seções do pack"]').$('a*=Problemas').click();
    await waitHeading('Problemas');
    const block = $('section[aria-labelledby]*=Desempenho do último teste');
    await block.$('dt=Tempo para abrir').waitForDisplayed({ timeout: 30_000 });
    const text = await block.getText();
    expect(text).toMatch(/Tempo para abrir\s*\d+ s\s*\+\d+% que na 1\.0\.0/);
    expect(text).toContain('Até entrar no mundo');
    expect(text).toContain('Sem perfil do spark neste teste.');
    await expect(block.$('a*=Perguntar ao Sculk por que ficou mais lento')).toBeDisplayed();
    expect(await seriousViolations()).toEqual([]);
    await screenshot('52-problemas-desempenho');
  });
});
