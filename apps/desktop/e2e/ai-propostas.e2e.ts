/**
 * ✦ Propostas da IA e consultas externas no app real (D-14; SPEC T14 ✦ passos 2 e 3), com o
 * Gemini simulado no roteiro das propostas (`PROPOSALS_MARKER`) e o GitHub simulado:
 *
 * - CA-T14-03 e CA-T14-13: com a caixa do GitHub marcada, o bloco "Enviado ao GitHub" é byte a
 *   byte o pedido que o GitHub recebeu, só com `repo:<dono>/<repo>` e as palavras do erro;
 * - CA-T14-11: a proposta com evidência inventada não aparece;
 * - CA-T14-12: as propostas não mudam nada até **Aplicar** (hash da pasta); **Descartar** não grava
 *   nada; Aplicar mostra a diferença, grava só aquela linha e cria o ponto de segurança;
 * - CA-T14-13: com a caixa desmarcada, nenhuma requisição vai ao GitHub.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { GEMINI_E2E_KEY, PROPOSALS_MARKER } from './mock-server/gemini';
import { openOrImport } from './open-or-import';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const PACK = join(DATA_ROOT, 'fora', 'pack-ia-propostas');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'gemini-api-key');
const PACK_NAME = 'Vale Proposto';
const CONFIG = '[geral]\nsegredoDaConfig = 42\noutra = 1\n';

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack Fabric 1.21.1 com o Sodium do Modrinth (o do servidor de fixtures) e uma config. */
function writePack(): void {
  mkdirSync(join(PACK, 'config'), { recursive: true });
  mkdirSync(join(PACK, 'mods'), { recursive: true });
  const sodium = [
    'name = "Sodium"',
    'filename = "sodium-fabric-0.6.0+mc1.21.1.jar"',
    'side = "client"',
    '',
    '[download]',
    'url = "https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium.jar"',
    'hash-format = "sha512"',
    `hash = "${'a'.repeat(128)}"`,
    '',
    '[update]',
    '[update.modrinth]',
    'mod-id = "AANobbMI"',
    'version = "SMxNOGZ6"',
    '',
  ].join('\n');
  writeFileSync(join(PACK, 'mods', 'sodium.pw.toml'), sodium);
  writeFileSync(join(PACK, 'config', 'exemplo.toml'), CONFIG);
  const index = [
    'hash-format = "sha256"',
    '',
    '[[files]]',
    'file = "config/exemplo.toml"',
    `hash = "${sha256(CONFIG)}"`,
    '',
    '[[files]]',
    'file = "mods/sodium.pw.toml"',
    `hash = "${sha256(sodium)}"`,
    'metafile = true',
    '',
  ].join('\n');
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      `name = "${PACK_NAME}"`,
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
      'fabric = "0.16.5"',
      'minecraft = "1.21.1"',
      '',
    ].join('\n'),
  );
}

/** Resumo de todos os arquivos da pasta do pack (caminho e conteúdo), com o repositório. */
function folderHash(dir: string): string {
  const hash = createHash('sha256');
  const walk = (current: string) => {
    for (const name of readdirSync(current).sort()) {
      const path = join(current, name);
      if (statSync(path).isDirectory()) {
        walk(path);
      } else {
        hash.update(path);
        hash.update(readFileSync(path));
      }
    }
  };
  walk(dir);
  return hash.digest('hex');
}

/** Os pontos de segurança do pack (referências `refs/warden/safety/…` do repositório escondido). */
function safetyPoints(): string[] {
  const out = execFileSync(
    'git',
    ['-C', PACK, 'for-each-ref', '--format=%(refname)', 'refs/warden/safety'],
    {
      encoding: 'utf8',
    },
  );
  return out.split('\n').filter((line) => line.trim() !== '');
}

async function requests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** Clica depois de fechar os avisos e de o alvo parar de mexer. */
async function clickStable(element: ReturnType<typeof $>): Promise<void> {
  await dismissToasts();
  await element.waitForClickable({ timeout: 30_000 });
  await element.waitForStable({ timeout: 10_000 });
  await element.click();
}

async function fullText(element: ReturnType<typeof $>): Promise<string> {
  await element.waitForExist({ timeout: 30_000 });
  const text = await element.getProperty('textContent');
  return typeof text === 'string' ? text : '';
}

/** Nova conversa sobre "só o pack", com o marcador do roteiro; `github`: a caixa do GitHub. */
async function startConversation(github: boolean): Promise<void> {
  const select = $('select.select');
  await select.waitForEnabled({ timeout: 30_000 });
  await select.selectByAttribute('value', 'pack');
  const note = $('textarea');
  await note.waitForEnabled({ timeout: 30_000 });
  await note.setValue(`${PROPOSALS_MARKER} trava ao entrar no mundo`);
  await clickStable($('main').$('button=Começar conversa'));
  const dialog = $('[role="dialog"]');
  await dialog.$('h2=Começar uma conversa com a IA?').waitForDisplayed({ timeout: 30_000 });
  const githubLabel = dialog.$('label*=no GitHub');
  await githubLabel.waitForDisplayed({ timeout: 10_000 });
  if (!github) await clickStable(githubLabel);
  await clickStable(dialog.$('button=Começar conversa'));
  await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
}

function card(title: string) {
  return $(`section.proposal[aria-labelledby]*=${title}`);
}

describe('✦ Propostas da IA (D-14)', function () {
  before(async function () {
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    mkdirSync(join(DATA_ROOT, 'cofre-de-teste'), { recursive: true });
    writeFileSync(KEY_FILE, GEMINI_E2E_KEY);
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await fetch(`${MOCK_URL}/__gemini/reset`, { method: 'POST' });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
  });

  after(() => {
    rmSync(KEY_FILE, { force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('CA-T14-03, CA-T14-11 e CA-T14-13: Enviado ao GitHub é o pedido exato; a proposta sem evidência não aparece', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    writeFileSync(PICK_FOLDER, PACK);
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
    await clickStable($('nav[aria-label="Seções do pack"]').$('a*=Diagnóstico com IA'));
    await waitHeading('Diagnóstico com IA');
    await $('h2=Nova conversa').waitForDisplayed({ timeout: 30_000 });
    await fetch(`${MOCK_URL}/__reset`);
    await startConversation(true);

    const block = $('[data-testid="enviado-github"]');
    await expect(block).toHaveText(
      expect.stringContaining('Enviado ao GitHub: busca de issues em CaffeineMC/sodium'),
    );
    await clickStable(block.$('summary'));
    const sent = await fullText(block.$('pre[aria-label="Pedido enviado ao GitHub"]'));
    const searches = (await requests()).filter((line) =>
      line.includes('/github/api/search/issues'),
    );
    expect(searches).toHaveLength(1);
    expect(sent).toBe(`GET ${MOCK_URL}${(searches[0] ?? '').replace(/^GET /, '')}`);
    const query = new URL(sent.replace(/^GET /, '')).searchParams.get('q');
    expect(query).toBe('repo:CaffeineMC/sodium is:issue NullPointerException EntityRenderer');

    await card('de 42 para 43').waitForDisplayed({ timeout: 30_000 });
    await expect(card('de 42 para 44')).toBeDisplayed();
    expect(await $$('section.proposal[aria-labelledby]').length).toBe(2);
    await expect($('section*=Remover Sodium')).not.toBeExisting();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d14-01-propostas');
  });

  it('CA-T14-12: nada muda até Aplicar; Descartar não grava; Aplicar grava só a linha com ponto de segurança', async () => {
    const untouched = folderHash(PACK);
    const pointsBefore = safetyPoints().length;

    const second = card('de 42 para 44');
    await clickStable(second.$('button*=Descartar'));
    await expect(second).toHaveText(expect.stringContaining('Descartada'));
    expect(folderHash(PACK)).toBe(untouched);

    const first = card('de 42 para 43');
    await clickStable(first.$('button*=Aplicar'));
    const dialog = $('[role="dialog"]');
    await dialog.$('h2=Aplicar a proposta?').waitForDisplayed({ timeout: 30_000 });
    await dialog.$('figure.diff').waitForDisplayed({ timeout: 60_000 });
    await expect(dialog).toHaveText(expect.stringContaining('segredoDaConfig = 43'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d14-02-aplicar');
    // Mostrar a diferença não grava nada.
    expect(folderHash(PACK)).toBe(untouched);

    await clickStable(dialog.$('button=Aplicar'));
    await dialog.waitForExist({ reverse: true, timeout: 60_000 });
    await expect(first).toHaveText(expect.stringContaining('ponto de segurança criado'));
    await browser.waitUntil(
      () => readFileSync(join(PACK, 'config', 'exemplo.toml'), 'utf8') !== CONFIG,
      { timeout: 30_000, timeoutMsg: 'a config não foi gravada' },
    );
    expect(readFileSync(join(PACK, 'config', 'exemplo.toml'), 'utf8')).toBe(
      CONFIG.replace('segredoDaConfig = 42', 'segredoDaConfig = 43'),
    );
    expect(safetyPoints().length).toBe(pointsBefore + 1);
    await screenshot('d14-03-aplicada');
  });

  it('CA-T14-13: com a caixa do GitHub desmarcada, nada vai ao GitHub', async () => {
    await clickStable($('a*=Conversas'));
    await waitHeading('Diagnóstico com IA');
    await fetch(`${MOCK_URL}/__reset`);
    await startConversation(false);
    await expect($('details.toolcall*=não permitido nesta conversa')).toBeDisplayed();
    expect((await requests()).some((line) => line.includes('/github/'))).toBe(false);
    await expect($('[data-testid="enviado-github"]')).not.toBeExisting();
  });
});
