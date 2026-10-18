/**
 * ✦ Diagnóstico com IA no app real (D-04; SPEC T14 ✦), com o Gemini simulado de
 * `mock-server/gemini.ts` (exige a chave só no cabeçalho e o histórico intacto, como a API real):
 *
 * - CA-T14-05: abrir o consentimento e cancelar não manda nada do pack ao Gemini (nenhum
 *   `generateContent`; o `countTokens` da prévia leva só as instruções do Warden);
 * - CA-T14-03: o texto inicial do consentimento e cada bloco "Enviado à IA" são byte a byte o que
 *   o Gemini recebeu; com a caixa de configs desmarcada, a config não sai ("não permitido nesta
 *   conversa");
 * - CA-T14-11: a afirmação com um trecho que a IA não recebeu aparece "não verificado";
 * - CA-T14-06: um log escolhido no computador passa pela mesma limpeza (nada pessoal no texto) e
 *   pelo mesmo consentimento, a conversa aparece em Conversas e a pasta do pack não muda;
 * - apagar a conversa tira a linha de Conversas.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const PICK_FILES = env('WARDEN_E2E_PICK_FILES');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const PACK = join(DATA_ROOT, 'fora', 'pack-ia');
const LOG_FILE = join(DATA_ROOT, 'fora', 'crash-escolhido.txt');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
/** Onde o cofre de teste (arquivo) guarda a chave do Gemini (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'gemini-api-key');
const PACK_NAME = 'Vale Consultado';

interface GeminiState {
  generate: string[];
  countTokens: string[];
  refusals: string[];
}

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 vazio, com uma config. */
function writePack(): void {
  mkdirSync(join(PACK, 'config'), { recursive: true });
  const index = 'hash-format = "sha256"\n';
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(join(PACK, 'config', 'exemplo.toml'), '[geral]\nsegredoDaConfig = 42\n');
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
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
}

/** Resumo de todos os arquivos da pasta do pack (caminho e conteúdo). */
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

async function gemini(): Promise<GeminiState> {
  const response = await fetch(`${MOCK_URL}/__gemini`);
  return (await response.json()) as GeminiState;
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

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

/** Texto completo de um elemento (inclusive o que está fora da tela ou recolhido). */
async function fullText(element: ReturnType<typeof $>): Promise<string> {
  return browser.execute((el: HTMLElement) => el.textContent, element as unknown as HTMLElement);
}

/** Escolhe a opção de "Começar pelo" pelo valor (sem pausa: espera a seleção valer). */
async function chooseSource(value: string): Promise<void> {
  const select = $('select.select');
  await select.waitForEnabled({ timeout: 30_000 });
  await select.selectByAttribute('value', value);
}

async function openConsent(): Promise<ReturnType<typeof $>> {
  await clickStable($('main').$('button=Começar conversa'));
  const dialog = $('[role="dialog"]');
  await dialog.$('h2=Começar uma conversa com a IA?').waitForDisplayed({ timeout: 30_000 });
  return dialog;
}

/** Espera a resposta da IA aparecer na conversa aberta. */
async function waitAnswer(): Promise<void> {
  await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
}

describe('✦ Diagnóstico com IA (D-04)', function () {
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
    rmSync(LOG_FILE, { force: true });
    writeFileSync(PICK_FILES, '');
  });

  it('CA-T14-05: o consentimento mostra o texto exato; cancelar não manda nada do pack', async () => {
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

    await clickStable(sectionLink('Diagnóstico com IA'));
    await waitHeading('Diagnóstico com IA');
    await $('h2=Nova conversa').waitForDisplayed({ timeout: 30_000 });
    // Sem teste gravado, "Começar pelo" vem em "Só o pack".
    await chooseSource('pack');
    const dialog = await openConsent();
    const text = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    expect(text).toContain(`Pack: ${PACK_NAME}, versão 1.0.0`);
    expect(text).toContain('Analisar: só o pack, sem log.');
    await expect(dialog).toHaveText(
      expect.stringContaining('Google Gemini (gemini-9.1-flash-lite)'),
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d04-01-consentimento');

    await clickStable(dialog.$('button=Cancelar'));
    await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
    const state = await gemini();
    expect(state.generate).toEqual([]);
    for (const body of state.countTokens) {
      expect(body).not.toContain(PACK_NAME);
    }
  });

  it('CA-T14-03 e CA-T14-11: o que saiu é o que a tela mostra; config negada; "não verificado"', async () => {
    await chooseSource('pack');
    const dialog = await openConsent();
    const shown = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    // Não deixar a IA ler configs.
    await clickStable(dialog.$('label*=Configs do pack'));
    await clickStable(dialog.$('button=Começar conversa'));
    await waitHeading('A visão geral do pack foi lida pela IA.');
    await waitAnswer();

    const state = await gemini();
    expect(state.refusals).toEqual([]);
    expect(state.generate.length).toBe(2);
    const first = JSON.parse(state.generate[0] ?? '{}') as {
      contents: { parts: { text: string }[] }[];
    };
    expect(first.contents[0]?.parts[0]?.text).toBe(shown);

    // Cada bloco "Enviado à IA", aberto, é um pedaço literal da requisição seguinte.
    expect(await $$('details.toolcall').length).toBe(4);
    for (const index of [1, 2, 3]) {
      const block = $$('details.toolcall')[index];
      if (!block) throw new Error(`bloco ${String(index)} ausente`);
      await clickStable(block.$('summary'));
      const sent = await fullText(block.$('pre[aria-label="Bytes enviados à IA"]'));
      expect(state.generate[1]).toContain(sent);
    }
    const denied = $('details.toolcall*=não permitido nesta conversa');
    await expect(denied).toHaveText(expect.stringContaining('config config/exemplo.toml'));
    expect(state.generate.join('\n')).not.toContain('segredoDaConfig');

    const verified = $('li.claim*=A visão geral do pack foi lida pela IA.');
    await expect(verified.$('button.evid')).toHaveText(
      expect.stringContaining('visão geral do pack'),
    );
    const unverified = $('li.claim*=O pack travou por falta de memória.');
    await expect(unverified).toHaveElementClass('claim--no');
    await expect(unverified).toHaveText(expect.stringContaining('não verificado'));
    await expect($('span*=Confiança média')).toBeDisplayed();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d04-02-conversa');
  });

  it('CA-T14-06: log do computador redigido, conversa em Conversas e a pasta do pack sem mudança', async () => {
    const before = folderHash(PACK);
    writeFileSync(
      LOG_FILE,
      [
        '[21:10:02] [main/INFO]: --gameDir C:\\Users\\Maria\\AppData\\Roaming\\x',
        '[21:11:41] [Server thread/INFO]: conectado de 192.168.0.42',
        '[21:14:07] [Server thread/ERROR]: java.lang.NoSuchMethodError: getPositionVec',
        '',
      ].join('\n'),
    );
    writeFileSync(PICK_FILES, LOG_FILE);
    await clickStable($('a*=Conversas'));
    await waitHeading('Diagnóstico com IA');
    await chooseSource('arquivo');
    await $('option*=Arquivo do computador: crash-escolhido.txt').waitForExist({ timeout: 30_000 });
    const dialog = await openConsent();
    const text = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    expect(text).toContain('NoSuchMethodError');
    expect(text).not.toContain('Maria');
    expect(text).not.toContain('192.168.0.42');
    await clickStable(dialog.$('button=Começar conversa'));
    await waitAnswer();
    const everything = (await gemini()).generate.join('\n');
    expect(everything).not.toContain('Maria');
    expect(everything).not.toContain('192.168.0.42');

    await clickStable($('a*=Conversas'));
    await waitHeading('Diagnóstico com IA');
    const rows = $('section[aria-labelledby="ia-conversas"]').$$('tbody tr');
    await browser.waitUntil(async () => (await rows.length) === 2, {
      timeout: 30_000,
      timeoutMsg: 'as duas conversas não apareceram em Conversas',
    });
    await expect($('td*=crash-escolhido.txt')).toBeDisplayed();
    expect(folderHash(PACK)).toBe(before);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d04-03-conversas');
  });

  it('apagar uma conversa tira a linha de Conversas', async () => {
    const rows = $('section[aria-labelledby="ia-conversas"]').$$('tbody tr');
    await clickStable($('section[aria-labelledby="ia-conversas"]').$('button*=Apagar a conversa'));
    const confirm = $('[role="alertdialog"]');
    await confirm.waitForDisplayed({ timeout: 10_000 });
    await clickStable(confirm.$('button=Apagar conversa'));
    await browser.waitUntil(async () => (await rows.length) === 1, {
      timeout: 30_000,
      timeoutMsg: 'a conversa apagada continuou em Conversas',
    });
  });
});
