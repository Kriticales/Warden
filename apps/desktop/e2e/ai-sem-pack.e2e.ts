/**
 * ✦ Analisar um log, a conversa com o Sculk sem pack, no app real (D-19; SPEC T14 item 8), com o
 * mclo.gs e o Gemini simulados (`mock-server/paste.ts` e `mock-server/gemini.ts`):
 *
 * - CA-T14-26: Meus packs abre "Analisar um log"; um link de outro site e o do hastebin mostram
 *   as frases de recusa, sem conversa nova;
 * - CA-T14-27 e CA-T14-28: colar um link do mclo.gs baixa só o texto dele; o consentimento mostra
 *   o texto já sem dados pessoais; cancelar não manda nada ao Gemini; o link não vai ao Sculk; o
 *   link do pastebin baixa só o texto puro dele (`pastebin.com/raw/<id>`);
 * - CA-T14-30: depois de **✦ Enviar e começar**, a resposta do Gemini simulado aparece; a
 *   conversa fica em `ai/_sem-pack/` nos dados do Warden, aparece na coluna de conversas da página
 *   e reabre igual depois de reiniciar o app;
 * - junção do lote 12 (D-16/D-17 × D-19): a mesma análise por link com a OpenAI e com o Claude
 *   simulados (`mock-server/openai.ts`, `anthropic.ts`), em "Procurar rápido": o consentimento
 *   diz o provedor e o modelo, a resposta cita a linha do log, e o pedido vai só ao provedor
 *   ativo, sem recusa.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { ANTHROPIC_E2E_KEY, ANTHROPIC_E2E_MODEL } from './mock-server/anthropic';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
import { OPENAI_E2E_KEY, OPENAI_E2E_MODEL } from './mock-server/openai';
import { MCLOGS_E2E_ID, PASTEBIN_E2E_ID } from './mock-server/paste';
import { useProviderFirst } from './keys';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
/** Onde o cofre de teste (arquivo) guarda a chave do Gemini (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'gemini-api-key');
/** As chaves da OpenAI e do Claude no mesmo cofre de teste. */
const OTHER_KEYS = [
  [join(DATA_ROOT, 'cofre-de-teste', 'openai-api-key'), OPENAI_E2E_KEY],
  [join(DATA_ROOT, 'cofre-de-teste', 'anthropic-api-key'), ANTHROPIC_E2E_KEY],
] as const;
/** O provedor ativo e a escolha de cada provedor (`ai/settings.json`). */
const AI_SETTINGS_FILE = join(DATA_ROOT, 'data', 'ai', 'settings.json');
/** As conversas sem pack, nos dados do Warden. */
const STANDALONE_DIR = join(DATA_ROOT, 'data', 'ai', '_sem-pack');
const LINK = `https://mclo.gs/${MCLOGS_E2E_ID}`;
const TITLE = 'O jogo travou num NoSuchMethodError.';

interface GeminiState {
  generate: string[];
  countTokens: string[];
  refusals: string[];
}

async function gemini(): Promise<GeminiState> {
  return mockState('gemini');
}

/** O que um dos simulados de IA recebeu (`/__gemini`, `/__openai`, `/__anthropic`). */
async function mockState(provider: 'gemini' | 'openai' | 'anthropic'): Promise<GeminiState> {
  const response = await fetch(`${MOCK_URL}/__${provider}`);
  return (await response.json()) as GeminiState;
}

/** Os caminhos pedidos ao servidor de fixtures desde o último `/__reset`. */
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

/** Texto completo de um elemento (inclusive o que está fora da tela ou recolhido). */
async function fullText(element: ReturnType<typeof $>): Promise<string> {
  await element.waitForExist({ timeout: 30_000 });
  const text = await element.getProperty('textContent');
  return typeof text === 'string' ? text : '';
}

/** Escreve o link no campo (apaga o que havia) e espera o valor valer. */
async function pasteLink(url: string): Promise<void> {
  const field = $('input[type="url"]');
  await field.waitForEnabled({ timeout: 30_000 });
  await field.clearValue();
  await field.setValue(url);
  await browser.waitUntil(async () => (await field.getValue()) === url, {
    timeout: 10_000,
    timeoutMsg: 'o link não ficou no campo',
  });
}

/** Espera o aviso de erro da página dizer `text`. */
async function waitAlert(text: string): Promise<void> {
  const alert = $('main [role="alert"]');
  await browser.waitUntil(async () => (await fullText(alert)).includes(text), {
    timeout: 30_000,
    timeoutMsg: `o aviso "${text}" não apareceu`,
  });
}

function beginButton() {
  return $('form.newconv').$('button*=Começar conversa');
}

/** "O que está acontecendo?" (sem ele, Começar conversa não anda). */
async function chooseSymptom(): Promise<void> {
  await clickStable($('form.newconv').$('button[role="radio"]*=Trava ao abrir o jogo'));
}

async function openConsent(): Promise<ReturnType<typeof $>> {
  await clickStable(beginButton());
  const dialog = $('[role="dialog"]');
  await dialog.$('h2*=Começar uma conversa com o Sculk?').waitForDisplayed({ timeout: 30_000 });
  return dialog;
}

/** O título da conversa aberta (a coluna do meio). */
async function waitTitle(text: string, timeout = 60_000): Promise<void> {
  await $('section.sculk__chat').$(`h2*=${text}`).waitForDisplayed({ timeout });
}

/** Espera a resposta do Sculk (o rodapé "O Sculk pode errar…"). */
async function waitAnswer(): Promise<void> {
  await $('.ai__foot*=O Sculk pode errar').waitForDisplayed({ timeout: 60_000 });
}

function standaloneRows() {
  return $('aside.sculk__convs').$$('a.conv');
}

describe('Analisar um log sem pack (D-19)', function () {
  before(async function () {
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    mkdirSync(join(DATA_ROOT, 'cofre-de-teste'), { recursive: true });
    writeFileSync(KEY_FILE, GEMINI_E2E_KEY);
    for (const [file, key] of OTHER_KEYS) writeFileSync(file, key);
    rmSync(AI_SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(STANDALONE_DIR, { recursive: true, force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    for (const provider of ['gemini', 'openai', 'anthropic']) {
      await fetch(`${MOCK_URL}/__${provider}/reset`, { method: 'POST' });
    }
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
  });

  after(() => {
    rmSync(KEY_FILE, { force: true });
    for (const [file] of OTHER_KEYS) rmSync(file, { force: true });
    rmSync(AI_SETTINGS_FILE, { force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(STANDALONE_DIR, { recursive: true, force: true });
  });

  it('CA-T14-26: Meus packs abre "Analisar um log"; outro site e o hastebin são recusados', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    await clickStable($('main').$('a*=Analisar um log'));
    await $('[data-testid="analisar-log"]').waitForDisplayed({ timeout: 60_000 });
    await $('section.sculk__chat').$('h2=Nova análise').waitForDisplayed({ timeout: 30_000 });
    await $('p=Nenhuma conversa sem pack ainda.').waitForDisplayed({ timeout: 30_000 });
    await chooseSymptom();

    await fetch(`${MOCK_URL}/__reset`);
    await pasteLink('https://example.com/abc123');
    await clickStable(beginButton());
    await waitAlert(
      'O Warden só abre links do mclo.gs e do pastebin. Escolha o arquivo do log ou cole um link de um desses sites.',
    );
    await pasteLink('https://hastebin.com/abcdef');
    await clickStable(beginButton());
    await waitAlert('O hastebin exige uma chave para ler os textos');
    expect(await $('[role="dialog"]').isExisting()).toBe(false);
    // Recusado sem rede: nenhum pedido ao site do link nem à IA.
    expect((await requests()).filter((path) => !path.includes('/gemini/'))).toEqual([]);
    expect((await gemini()).generate).toEqual([]);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d19-01-recusa');
  });

  it('CA-T14-26: o link do pastebin baixa só o texto puro dele', async () => {
    await fetch(`${MOCK_URL}/__reset`);
    await pasteLink(`https://pastebin.com/${PASTEBIN_E2E_ID}`);
    const dialog = await openConsent();
    const text = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    expect(text).toContain('link do pastebin');
    expect(text).toContain('NoSuchMethodError');
    expect(text).not.toContain('Maria');
    expect(text).not.toContain(PASTEBIN_E2E_ID);
    expect((await requests()).filter((path) => path.includes('/pastebin/'))).toEqual([
      `GET /pastebin/raw/${PASTEBIN_E2E_ID}`,
    ]);
    await clickStable(dialog.$('button=Cancelar'));
    await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
    expect((await gemini()).generate).toEqual([]);
  });

  it('CA-T14-27 e CA-T14-28: o link baixa só o texto, sem dados pessoais; cancelar não manda nada', async () => {
    await fetch(`${MOCK_URL}/__reset`);
    await pasteLink(LINK);
    const dialog = await openConsent();
    const text = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    expect(text).toContain('NoSuchMethodError');
    expect(text).toContain('link do mclo.gs');
    expect(text).not.toContain('Maria');
    expect(text).not.toContain('192.168.0.42');
    expect(text).not.toContain(MCLOGS_E2E_ID);
    expect((await requests()).filter((path) => path.includes('/mclogs/'))).toEqual([
      `GET /mclogs/1/raw/${MCLOGS_E2E_ID}`,
    ]);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d19-02-consentimento');

    await clickStable(dialog.$('button=Cancelar'));
    await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
    const state = await gemini();
    expect(state.generate).toEqual([]);
    for (const body of state.countTokens) {
      expect(body).not.toContain('NoSuchMethodError');
    }
    await browser.waitUntil(async () => await beginButton().isFocused(), {
      timeout: 10_000,
      timeoutMsg: 'o foco não voltou para Começar conversa',
    });
  });

  it('CA-T14-30: a resposta do Gemini chega; a conversa fica guardada e reabre depois de reiniciar', async () => {
    const dialog = await openConsent();
    const shown = await fullText(dialog.$('pre[aria-label="Texto inicial que será enviado"]'));
    await clickStable(dialog.$('button*=Enviar e começar'));
    await waitTitle(TITLE);
    await waitAnswer();

    const state = await gemini();
    expect(state.refusals).toEqual([]);
    expect(state.generate.length).toBe(2);
    const first = JSON.parse(state.generate[0] ?? '{}') as {
      contents: { parts: { text: string }[] }[];
    };
    expect(first.contents[0]?.parts[0]?.text).toBe(shown);
    const everything = state.generate.join('\n');
    expect(everything).not.toContain('Maria');
    expect(everything).not.toContain('192.168.0.42');
    expect(everything).not.toContain(MCLOGS_E2E_ID);
    await clickStable($('details.consults').$('summary'));
    await expect($('details.consults').$('details.tc*=no log do link')).toBeDisplayed();
    await expect($(`li.claim*=${TITLE}`).$('button.evid')).toHaveText(
      expect.stringContaining('log, linha 3'),
    );
    // Sem pack: nem propostas nem "Encontrar o mod culpado".
    expect(await $('a=Encontrar o mod culpado').isExisting()).toBe(false);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d19-03-conversa');

    // Guardada nos dados do Warden, fora de qualquer pack.
    await browser.waitUntil(
      () =>
        existsSync(STANDALONE_DIR) &&
        readdirSync(STANDALONE_DIR).some((name) => name.endsWith('.json')),
      { timeout: 10_000, timeoutMsg: 'a conversa não foi gravada em ai/_sem-pack/' },
    );

    // "Reiniciar o app" e reabrir pela lista.
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    await clickStable($('main').$('a*=Analisar um log'));
    await $('[data-testid="analisar-log"]').waitForDisplayed({ timeout: 60_000 });
    await browser.waitUntil(async () => (await standaloneRows().length) === 1, {
      timeout: 30_000,
      timeoutMsg: 'a conversa não apareceu em Conversas sem pack',
    });
    await expect($('aside.sculk__convs').$('a.conv*=Link do mclo.gs')).toBeDisplayed();
    await clickStable($('aside.sculk__convs').$('a.conv'));
    await waitTitle(TITLE);
    await waitAnswer();
    expect((await gemini()).generate.length).toBe(2);
    await screenshot('d19-04-reaberta');
  });

  for (const [provider, radio, model] of [
    ['openai', 'OpenAI', OPENAI_E2E_MODEL],
    ['anthropic', 'Claude', ANTHROPIC_E2E_MODEL],
  ] as const) {
    it(`junção D-16/D-17 × D-19: a análise por link funciona com ${radio}`, async () => {
      // Configurações › Sensor Sculk: este provedor vai para o primeiro da ordem e abre as
      // conversas novas (a chave já está no cofre; ADR-0070). A barra de cima tem
      // Configurações em qualquer tela (aqui, a conversa do caso anterior).
      await useProviderFirst(provider);

      const before = {
        gemini: (await mockState('gemini')).generate.length,
        openai: (await mockState('openai')).generate.length,
        anthropic: (await mockState('anthropic')).generate.length,
      };
      await clickStable($('a*=Meus packs'));
      await waitHeading('Meus packs');
      await clickStable($('main').$('a*=Analisar um log'));
      await $('[data-testid="analisar-log"]').waitForDisplayed({ timeout: 60_000 });
      await clickStable($('aside.sculk__convs').$('button*=Nova conversa'));
      await $('section.sculk__chat').$('h2=Nova análise').waitForDisplayed({ timeout: 30_000 });
      await chooseSymptom();
      // "Procurar rápido": o modelo mais econômico da regra (o que o simulado espera).
      await clickStable($('form.newconv').$('button[role="radio"]=Procurar rápido'));
      await pasteLink(LINK);
      const dialog = await openConsent();
      await expect(dialog).toHaveText(
        expect.stringContaining(`${radio} (${model}), com a sua chave`),
      );
      await expect(dialog).not.toHaveText(expect.stringContaining('plano gratuito'));
      await clickStable(dialog.$('button*=Enviar e começar'));
      await waitTitle(TITLE);
      await waitAnswer();
      await expect($(`span.sculk__meta*=${radio} ·`)).toBeDisplayed();
      await expect($(`li.claim*=${TITLE}`).$('button.evid')).toHaveText(
        expect.stringContaining('log, linha 3'),
      );
      expect(await seriousViolationSummaries()).toEqual([]);
      await screenshot(`d19-05-conversa-${provider}`);

      const state = await mockState(provider);
      expect(state.refusals).toEqual([]);
      expect(state.generate.length).toBe(before[provider] + 2);
      const everything = state.generate.slice(before[provider]).join('\n');
      expect(everything).not.toContain('Maria');
      expect(everything).not.toContain(MCLOGS_E2E_ID);
      for (const other of ['gemini', 'openai', 'anthropic'] as const) {
        if (other !== provider) {
          expect((await mockState(other)).generate.length).toBe(before[other]);
        }
      }
    });
  }
});
