/**
 * "Testar com perfil de desempenho" no app real (L-12; SPEC T13; CA-T13-13), com o jogo simulado
 * da `warden-launcher` (fica aberto com um filho). Com o jogo aberto, o E2E faz o que o Forge
 * 1.20.1 e o spark fariam na instância de teste: grava o `logs/debug.log` com o registro detalhado
 * e um perfil salvo pelo spark (`config/spark/*.sparkprofile`), os dois reais
 * (`crates/warden-perf/tests/`, origem em `FIXTURES.md`).
 *
 * - O item "Testar com perfil de desempenho" do menu ▾ começa o teste.
 * - No resultado, a parte "Desempenho" mostra o que mais demorou para carregar (JEI primeiro) e
 *   o resumo do perfil do spark; nada do spark aparece em "O que mudou durante o teste".
 * - "Abrir no visualizador do spark" abre o visualizador (aqui, o endereço vai para o arquivo
 *   de `WARDEN_E2E_OPEN_URL`) com o arquivo da sessão; sem violações sérias do axe.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { dismissToasts } from './toasts';
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
const OPENED_FILE = env('WARDEN_E2E_OPEN_URL');
const PACK = join(DATA_ROOT, 'fora', 'pack-perfil-desempenho');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const INSTANCES = join(DATA_ROOT, 'data', 'instances');
const GOLDEN = join(import.meta.dirname, '..', '..', '..', 'crates', 'warden-perf', 'tests');
/** Arquivo do pack que marca a instância deste E2E. */
const MARKER = 'config/marcador-perfil.toml';
const PROFILE = 'profile-2026-10-18_00.42.48.sparkprofile';

const RESULT = 'section[aria-labelledby="perf-report-title"]';

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack Forge 1.20.1 com uma config (o marcador) e o modo do jogo simulado. */
function writePack(): void {
  mkdirSync(join(PACK, 'config'), { recursive: true });
  const marker = 'perfil = true\n';
  writeFileSync(join(PACK, MARKER), marker);
  const index = [
    'hash-format = "sha256"',
    '',
    '[[files]]',
    `file = "${MARKER}"`,
    `hash = "${sha256(marker)}"`,
    '',
  ].join('\n');
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale com Perfil"',
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
      'forge = "47.4.10"',
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, '.warden-fake-game'), 'filhos 1');
}

/** A instância de teste deste pack (a que recebeu o marcador). */
function instanceGameDir(): string {
  for (const id of readdirSync(INSTANCES)) {
    const game = join(INSTANCES, id, 'minecraft');
    if (existsSync(join(game, MARKER))) return game;
  }
  throw new Error('instância de teste do pack do perfil ausente');
}

/** O "jogo" grava um arquivo real na instância. */
function play(relative: string, golden: string): void {
  const file = join(instanceGameDir(), relative);
  mkdirSync(join(file, '..'), { recursive: true });
  writeFileSync(file, readFileSync(join(GOLDEN, golden)));
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 120_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** O texto visível de uma parte da tela (pelo `innerText`, como no E2E do desempenho). */
async function textOf(selector: string): Promise<string> {
  return browser.execute(
    (css: string) => document.querySelector<HTMLElement>(css)?.innerText ?? '',
    selector,
  );
}

describe('Testar com perfil de desempenho (L-12; CA-T13-13)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do perfil de desempenho pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(OPENED_FILE, { force: true });
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
        rmSync(join(INSTANCES, pack.id), { recursive: true, force: true });
      }
    } catch {
      // Sem registro: nada foi aberto.
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(OPENED_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('o teste mede o tempo por mod e resume o perfil do spark salvo no jogo', async () => {
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

    await dismissToasts();
    await $('header.packhead .testbtn__more').click();
    const item = $('[role="menuitem"]*=Testar com perfil de desempenho');
    await item.waitForDisplayed();
    await expect(item).toHaveText(
      expect.stringContaining('Mede o que mais pesa para carregar e rodar'),
    );
    await item.click();
    await waitHeading('Jogo aberto');
    const gameConsole = $('section[aria-label="Console do jogo"]');
    await browser.waitUntil(async () => (await gameConsole.getText()).includes('pai vivo'), {
      timeout: 30_000,
      timeoutMsg: 'o console não mostrou o jogo simulado',
    });

    // O Forge grava o debug.log com o registro detalhado; a pessoa salva um perfil no spark.
    play('logs/debug.log', 'logs/forge-1.20.1-servidor-debug.log');
    play(`config/spark/${PROFILE}`, 'spark/forge-1.20.1-servidor.sparkprofile');

    await dismissToasts();
    await $('main').$('button=Parar jogo').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await dialog.$('button=Parar jogo').click();
    await waitHeading('Encerrado por você', 30_000);
    await $(RESULT).waitForDisplayed({ timeout: 30_000 });

    let text = '';
    await browser.waitUntil(
      async () => {
        text = await textOf(RESULT);
        return text.includes('Perfil do spark salvo às');
      },
      { timeout: 15_000, timeoutMsg: 'o resumo do spark não apareceu' },
    );
    expect(text).toContain('Teste com perfil de desempenho');
    expect(text).toContain('O que mais demorou para carregar');
    expect(text).toMatch(/jei\s+288 ms/);
    expect(text).toMatch(/mezz_config\s+60 ms/);
    expect(text).toContain('O Minecraft e o Forge levaram 350 ms nessas fases.');
    expect(text).toContain('1,6 ms na mediana, 213,7 ms no pior tick');
    expect(text).toContain('spark (1,7%)');
    // "O que mudou durante o teste": nada do spark (nem do log).
    const page = await textOf('main');
    expect(page).not.toContain('config/spark');
    expect(page).not.toContain('.sparkprofile');
    await screenshot('60-perfil-de-desempenho');
    expect(await seriousViolations()).toEqual([]);

    await dismissToasts();
    const viewer = $(RESULT).$('button=Abrir no visualizador do spark');
    await viewer.waitForClickable();
    await viewer.click();
    await browser.waitUntil(() => existsSync(OPENED_FILE), {
      timeout: 15_000,
      timeoutMsg: 'o visualizador do spark não foi aberto',
    });
    expect(readFileSync(OPENED_FILE, 'utf8')).toBe(`https://spark.lucko.me/\n${PROFILE}\n`);
  });
});
