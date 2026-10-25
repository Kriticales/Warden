/**
 * "O que mudou durante o teste" no app real (C-03; SPEC T15), com o jogo simulado da
 * `warden-launcher` (modo `filhos`: fica aberto até "Parar jogo"). Enquanto o jogo está aberto, o
 * teste muda arquivos da instância como o jogo mudaria; ao parar, o resultado mostra a captura.
 *
 * - CA-T15-01: mudar `dificuldade` numa config do pack faz aparecer só essa chave; trazer grava
 *   só essa linha no pack.
 * - CA-T15-07 (ADR-0067): o `options.txt` é pessoal: mudar a distância de renderização não
 *   aparece, ele nunca vai para o pack e volta igual no teste seguinte, mesmo apagado.
 * - CA-T15-02: uma config só reformatada não aparece.
 * - Configs (T12, P9): depois do teste, a seção abre no teste local com os três grupos e o
 *   `options.txt` em "Só neste computador".
 * - CA-T15-03 e CA-T15-06: arquivos sempre ignorados (logs, spark, KubeJS, ProbeJS) não
 *   aparecem.
 * - Testar de novo com mudanças não revisadas pergunta antes; "Descartar e testar" segue.
 * - O cabeçalho avisa "Mudanças do teste para revisar" depois de "Decidir depois".
 *
 * Sem pausas fixas: cada passo espera a condição real. Com `WARDEN_E2E_SCREENSHOTS=<pasta>`,
 * grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { dismissToasts } from './toasts';
import { openOrImport } from './open-or-import';
import { stopGame } from './steps';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'Pack da captura ç');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

const OPTIONS = 'version:3465\nrenderDistance:16\nguiScale:0\nfullscreen:false\n';
const CONFIG = '# Config do pack\n[geral]\n\tdificuldade = 2\n\tnome = "Vale"\n';
const BROUGHT = CONFIG.replace('dificuldade = 2', 'dificuldade = 3');
const MARKER = 'config/captura-e2e.toml';
const REFORMAT = 'config/reformatada.toml';
const REFORMAT_TEXT = '[geral]\n\tnome = "Vale"\n';

function sha256(data: string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 com os arquivos dados no índice e o jogo simulado aberto. */
function writePack(files: Record<string, string>): void {
  mkdirSync(PACK, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, text]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, text);
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(text)}"\n`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Pack da Captura"',
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
  writeFileSync(join(PACK, '.warden-fake-game'), 'filhos 1');
}

/** A instância de teste deste pack (a que recebeu o arquivo marcador). */
function instanceGameDir(): string {
  const instances = join(DATA_ROOT, 'data', 'instances');
  for (const id of readdirSync(instances)) {
    const game = join(instances, id, 'minecraft');
    if (existsSync(join(game, MARKER))) return game;
  }
  throw new Error('instância de teste do pack da captura ausente');
}

/** A cópia pessoal guardada pelo Warden (ADR-0067), do único pack com configs pessoais. */
function personalCopy(relative: string): string {
  const root = join(DATA_ROOT, 'data', 'personal');
  for (const id of readdirSync(root)) {
    const file = join(root, id, 'files', relative);
    if (existsSync(file)) return file;
  }
  throw new Error(`cópia pessoal de ${relative} ausente`);
}

/** O jogo grava um arquivo na instância. */
function play(relative: string, text: string): void {
  const file = join(instanceGameDir(), relative);
  mkdirSync(join(file, '..'), { recursive: true });
  writeFileSync(file, text);
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function testButton() {
  return $('header.packhead .testbtn__main');
}

/** Espera o texto aparecer na área principal. */
async function waitText(text: string): Promise<void> {
  await browser.waitUntil(async () => (await $('main').getText()).includes(text), {
    timeout: 15_000,
    timeoutMsg: `não apareceu: ${text}`,
  });
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

function capture() {
  return $('section.capture');
}

/** Testar, esperar o jogo simulado abrir, mudar arquivos e parar. */
async function playAndStop(changes: () => void): Promise<void> {
  await dismissToasts();
  await testButton().click();
  await waitHeading('Jogo aberto');
  const console = $('section[aria-label="Console do jogo"]');
  await browser.waitUntil(async () => (await console.getText()).includes('pai vivo'), {
    timeout: 30_000,
    timeoutMsg: 'o console não mostrou o jogo simulado',
  });
  changes();
  await stopGame();
}

describe('O que mudou durante o teste (C-03; T15)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E da captura pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack({ [MARKER]: CONFIG, [REFORMAT]: REFORMAT_TEXT });
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(REGISTRY_FILE, { force: true });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    // Os outros specs (o do Testar) contam com uma instância só em `instances/`.
    if (existsSync(join(DATA_ROOT, 'data', 'instances'))) {
      rmSync(join(instanceGameDir(), '..'), { recursive: true, force: true });
    }
  });

  it('abre o pack', async () => {
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
  });

  it('CA-T15-01, -02, -03, -06 e -07: só a mudança de verdade aparece; options.txt nunca', async () => {
    await playAndStop(() => {
      play(MARKER, BROUGHT);
      // Preferência pessoal (ADR-0067): nunca é candidata ao pack.
      play('options.txt', OPTIONS);
      // Reformatada pelo jogo: mesmos valores.
      play(REFORMAT, '[geral]\r\n    nome = "Vale"\r\n');
      // Sempre ignorados.
      play('logs/latest.log', 'linha');
      play('config/spark/perfil.sparkprofile', 'perfil');
      play('kubejs/config/web_server.json', '{"token":"x"}');
      play('.probe/types.d.ts', 'tipos');
      play('local/kubejs/x.json', '{}');
    });
    const block = capture();
    await block.waitForDisplayed({ timeout: 15_000 });
    await expect(block).toHaveText(expect.stringContaining('Durante o teste, 1 arquivo mudou'));
    await expect(block).toHaveText(expect.stringContaining('geral.dificuldade'));
    const text = await block.getText();
    for (const hidden of [
      'options.txt',
      'renderDistance',
      'reformatada.toml',
      'latest.log',
      'sparkprofile',
      'web_server',
      '.probe',
      'local/kubejs',
    ]) {
      expect(text).not.toContain(hidden);
    }
    await expect(block).toHaveText(
      expect.stringContaining('1 arquivo só foi reformatado pelo jogo'),
    );
    expect(await block.$(`input[aria-label="Trazer ${MARKER} para o pack"]`).isSelected()).toBe(
      true,
    );
    expect(await seriousViolations()).toEqual([]);
    await screenshot('c03-o-que-mudou');
  });

  it('"Decidir depois" leva a Mods e o cabeçalho avisa; o aviso volta ao resultado', async () => {
    await dismissToasts();
    await capture().$('button=Decidir depois').click();
    await waitHeading('Mods');
    const alert = $('header.packhead').$('button=Mudanças do teste para revisar');
    await alert.waitForDisplayed();
    await screenshot('c03-aviso-cabecalho');
    await alert.click();
    await capture().waitForDisplayed({ timeout: 15_000 });
  });

  it('Testar com mudanças não revisadas pergunta antes', async () => {
    await dismissToasts();
    await testButton().click();
    await waitHeading('Mudanças do último teste para revisar');
    await expect($('main').$('button=Revisar agora')).toBeDisplayed();
    await expect($('main').$('button=Descartar e testar')).toBeDisplayed();
    expect(await seriousViolations()).toEqual([]);
    await screenshot('c03-pergunta-antes');
    await $('main').$('button=Revisar agora').click();
    await capture().waitForDisplayed({ timeout: 15_000 });
  });

  it('CA-T15-01: trazer grava só a linha da dificuldade; o options.txt fica só aqui', async () => {
    await dismissToasts();
    const bring = capture().$('button=Trazer 1 selecionado para o pack');
    await bring.waitForClickable();
    await bring.click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('1 mudança trazida para o pack'));
    await screenshot('c03-trazido');
    await dialog.$('button=Continuar editando').click();
    await waitText('1 mudança foi trazida para o pack');
    expect(readFileSync(join(PACK, MARKER), 'utf8')).toBe(BROUGHT);
    // ADR-0067: o options.txt nunca vai para o pack; a cópia pessoal ficou nos dados do Warden.
    expect(existsSync(join(PACK, 'options.txt'))).toBe(false);
    expect(readFileSync(personalCopy('options.txt'), 'utf8')).toBe(OPTIONS);
  });

  it('Descartar: a config volta à do pack; o options.txt volta igual mesmo apagado', async () => {
    // Descartar apaga da instância as configs do pack que mudaram (a próxima cópia as grava de
    // novo): a pasta do jogo fica guardada antes.
    const game = instanceGameDir();
    await playAndStop(() => {
      play(MARKER, BROUGHT.replace('dificuldade = 3', 'dificuldade = 4'));
    });
    const block = capture();
    await block.waitForDisplayed({ timeout: 15_000 });
    await expect(block).toHaveText(expect.stringContaining('geral.dificuldade'));
    await dismissToasts();
    await block.$('button=Descartar tudo').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await dialog.$('button=Descartar mudanças').click();
    await waitText('Mudanças deste teste descartadas');
    // Apagada na instância: o próximo teste devolve a cópia guardada (CA-T15-07).
    rmSync(join(game, 'options.txt'));
    await playAndStop(() => undefined);
    expect(readFileSync(join(game, MARKER), 'utf8')).toBe(BROUGHT);
    expect(readFileSync(join(game, 'options.txt'), 'utf8')).toBe(OPTIONS);
    expect(existsSync(join(PACK, 'options.txt'))).toBe(false);
  });

  it('Configs: o teste local com No pack, Só no teste e Só neste computador; axe', async () => {
    await dismissToasts();
    // O jogo criou uma config que nunca foi para o pack.
    play('config/criada-pelo-jogo.toml', 'ligado = true\n');
    await $('nav[aria-label="Seções do pack"]').$('a*=Configs').click();
    await $('h1=Configs').waitForDisplayed();
    const pack = $('[role="tree"][aria-label="Configs que estão no pack"]');
    await pack.waitForDisplayed({ timeout: 15_000 });
    expect(await $('[role="radio"][aria-checked="true"]').getText()).toContain('Do teste local');
    await expect(pack).toHaveElementProperty(
      'textContent',
      expect.stringContaining('captura-e2e.toml'),
    );
    const onlyTest = $('[role="tree"][aria-label="Configs que estão só no teste"]');
    await expect(onlyTest).toHaveElementProperty(
      'textContent',
      expect.stringContaining('criada-pelo-jogo.toml'),
    );
    const personal = $('[role="tree"][aria-label="Preferências pessoais"]');
    await expect(personal).toHaveElementProperty(
      'textContent',
      expect.stringContaining('Opções do jogo'),
    );
    await personal.$('[role="treeitem"]*=Opções do jogo').click();
    await $('.band*=Preferência pessoal.').waitForDisplayed();
    expect(await seriousViolations()).toEqual([]);
    await screenshot('c03-configs-tres-grupos');
  });
});
