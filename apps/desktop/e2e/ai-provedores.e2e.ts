/**
 * IA com vários provedores no app real (D-18; ADR-0060; SPEC T14 ✦ e T21), com os servidores
 * simulados de `mock-server/` (o do Gemini exige a chave só no cabeçalho e o histórico intacto,
 * como a API real):
 *
 * - CA-T21-06 (ADR-0070): em Chaves e contas, a chave do Gemini salva pela tela é testada na
 *   hora e fica "Funcionando"; em Configurações → Sensor Sculk, o Gemini abre as conversas
 *   novas, com o modelo; pôr a OpenAI (sem chave) em primeiro não muda o provedor das conversas
 *   novas ("Pulado: sem chave"); reiniciar mantém a ordem; nenhuma chave aparece em
 *   `settings.json` nem em `ai/settings.json` (varredura);
 * - CA-T14-20 (Gemini): a conversa nova usa o provedor escolhido; o consentimento mostra o nome
 *   do provedor e o modelo, com o aviso de plano gratuito;
 * - CA-T14-21 (telas): mudar a ordem não muda a conversa já começada, que continua abrindo com
 *   o Gemini; sem a chave do Gemini, ela pede essa chave e leva a Configurações, e perguntar de
 *   novo mostra qual chave falta, sem nenhuma requisição de rede.
 *
 * - CA-T14-20 (OpenAI e Claude, ligados na junção do lote 12 com os simulados e os tradutores da
 *   D-16/D-17): a chave pela tela, o provedor em primeiro na ordem, o modelo da regra na seção
 *   e no consentimento, sem plano gratuito, e a conversa nova só nesse provedor.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { ANTHROPIC_E2E_KEY, ANTHROPIC_E2E_MODEL } from './mock-server/anthropic';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
import { OPENAI_E2E_KEY, OPENAI_E2E_MODEL } from './mock-server/openai';
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
const PACK = join(DATA_ROOT, 'fora', 'pack-provedores');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
/** O `ai/settings.json` (provedor ativo e escolha de cada provedor). */
const AI_SETTINGS_FILE = join(DATA_ROOT, 'data', 'ai', 'settings.json');
/** Onde o cofre de teste (arquivo) guarda a chave do Gemini (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'gemini-api-key');
/** As chaves da OpenAI e do Claude no mesmo cofre de teste. */
const OTHER_KEY_FILES = ['openai-api-key', 'anthropic-api-key'].map((name) =>
  join(DATA_ROOT, 'cofre-de-teste', name),
);
const PACK_NAME = 'Vale dos Provedores';

interface GeminiState {
  generate: string[];
  countTokens: string[];
  refusals: string[];
}

function sha256(data: string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 vazio. */
function writePack(): void {
  mkdirSync(PACK, { recursive: true });
  const index = 'hash-format = "sha256"\n';
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
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
}

async function gemini(): Promise<GeminiState> {
  const response = await fetch(`${MOCK_URL}/__gemini`);
  return (await response.json()) as GeminiState;
}

type AiProvider = 'gemini' | 'openai' | 'anthropic';
const AI_PROVIDERS: readonly AiProvider[] = ['gemini', 'openai', 'anthropic'];
/** O caminho do estado de cada simulado (`/__gemini`, `/__openai`, `/__anthropic`). */
const MOCK_PATH: Record<AiProvider, string> = {
  gemini: 'gemini',
  openai: 'openai',
  anthropic: 'anthropic',
};

/** O que os três simulados receberam até agora. */
async function allProviders(): Promise<
  Record<AiProvider, { generate: string[]; refusals: string[] }>
> {
  const entries = await Promise.all(
    AI_PROVIDERS.map(async (provider) => {
      const response = await fetch(`${MOCK_URL}/__${MOCK_PATH[provider]}`);
      return [provider, (await response.json()) as GeminiState] as const;
    }),
  );
  return Object.fromEntries(entries) as Record<AiProvider, GeminiState>;
}

/** "do Gemini", "da OpenAI", "do Claude" (o rótulo do campo da chave). */
function keyLabel(provider: AiProvider): string {
  return { gemini: 'do Gemini', openai: 'da OpenAI', anthropic: 'do Claude' }[provider];
}

/** O nome na tela: Gemini, OpenAI, Claude. */
function providerName(provider: AiProvider): string {
  return { gemini: 'Gemini', openai: 'OpenAI', anthropic: 'Claude' }[provider];
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

/** O axe com a tela parada: os avisos (toasts) da troca de provedor fechados antes. */
async function axeAfterToasts(): Promise<string[]> {
  await dismissToasts();
  return seriousViolationSummaries();
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

const keyItem = (kind: string) => $(`[data-testid="chave-${kind}"]`);
/** A linha de um provedor na ordem de Configurações → Sensor Sculk. */
const orderRow = (provider: AiProvider) => $(`.porder__row[data-provider="${provider}"]`);

/** Meus packs, de onde estiver (a volta do pack ou de Configurações). */
async function openMyPacks(): Promise<void> {
  if (!(await $('h1=Meus packs').isDisplayed())) {
    await clickStable($('a*=Meus packs'));
  }
  await waitHeading('Meus packs');
}

/** Configurações → uma página do menu lateral. */
async function openSettings(page: 'Chaves e contas' | 'Sensor Sculk'): Promise<void> {
  await openMyPacks();
  await clickStable($('header.topbar').$('a*=Configurações'));
  await waitHeading('Geral');
  await clickStable($('nav.sidenav').$(`a*=${page}`));
  await waitHeading(page);
}

/** Salva a chave do provedor pela tela; o Warden testa na hora e ela fica "Funcionando". */
async function saveKey(provider: AiProvider, key: string): Promise<void> {
  await openSettings('Chaves e contas');
  await clickStable(keyItem(provider));
  const detail = $('section.keydet');
  await detail.$(`input[type="password"]`).waitForDisplayed({ timeout: 30_000 });
  await expect($(`label=Chave ${keyLabel(provider)}`)).toBeDisplayed();
  await detail.$('input[type="password"]').setValue(key);
  await clickStable(detail.$('button=Salvar'));
  await keyItem(provider).$('span*=Funcionando').waitForDisplayed({ timeout: 30_000 });
}

/** Põe o provedor em primeiro na ordem (pelas setas) e espera cada passo ser gravado. */
async function moveToTop(provider: AiProvider): Promise<void> {
  await openSettings('Sensor Sculk');
  for (;;) {
    const order = aiSettings().aiOrder ?? [...AI_PROVIDERS];
    const index = order.indexOf(provider);
    if (index <= 0) break;
    await clickStable(orderRow(provider).$(`button[aria-label="Subir ${providerName(provider)}"]`));
    await browser.waitUntil(() => (aiSettings().aiOrder ?? []).indexOf(provider) === index - 1, {
      timeout: 30_000,
      timeoutMsg: 'a ordem não foi gravada',
    });
  }
}

/** Meus packs → o pack → ✦ Diagnóstico com IA. */
async function openAiSection(): Promise<void> {
  await openMyPacks();
  await clickStable($(`tr.packrow*=${PACK_NAME}`).$('button.packrow__title'));
  await waitHeading('Mods');
  await clickStable($('nav[aria-label="Seções do pack"]').$('a*=Diagnóstico com IA'));
  await waitHeading('Diagnóstico com IA');
}

function aiSettings(): { aiProvider?: string; aiOrder?: string[] } {
  if (!existsSync(AI_SETTINGS_FILE)) return {};
  return JSON.parse(readFileSync(AI_SETTINGS_FILE, 'utf8')) as {
    aiProvider?: string;
    aiOrder?: string[];
  };
}

describe('IA com vários provedores (D-18)', function () {
  before(async function () {
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    for (const file of [KEY_FILE, ...OTHER_KEY_FILES]) rmSync(file, { force: true });
    rmSync(AI_SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    for (const provider of AI_PROVIDERS) {
      await fetch(`${MOCK_URL}/__${MOCK_PATH[provider]}/reset`, { method: 'POST' });
    }
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
  });

  after(() => {
    for (const file of [KEY_FILE, ...OTHER_KEY_FILES]) rmSync(file, { force: true });
    rmSync(AI_SETTINGS_FILE, { force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('CA-T21-06 e CA-T14-20 (Gemini): a chave pela tela, o Testar, a linha do Gemini e o consentimento com o provedor', async () => {
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

    await saveKey('gemini', GEMINI_E2E_KEY);
    await expect($('section.keydet')).toHaveText(expect.stringContaining('1º na preferência'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-01-chaves-gemini');

    await openSettings('Sensor Sculk');
    const geminiRow = orderRow('gemini');
    await geminiRow.$('span*=Conversas novas').waitForDisplayed({ timeout: 30_000 });
    await geminiRow.$('b=gemini-9.1-flash').waitForDisplayed({ timeout: 30_000 });
    await expect(geminiRow).toHaveText(expect.stringContaining('gemini-9.1-flash-lite'));
    await expect(orderRow('openai')).toHaveText(expect.stringContaining('Pulado: sem chave'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-01-sensor-sculk-gemini');

    await openAiSection();
    await $('h2=Nova conversa').waitForDisplayed({ timeout: 30_000 });
    await $('select.select').selectByAttribute('value', 'pack');
    await clickStable($('main').$('button=Começar conversa'));
    const dialog = $('[role="dialog"]');
    await dialog.$('h2=Começar uma conversa com a IA?').waitForDisplayed({ timeout: 30_000 });
    await expect(dialog).toHaveText(
      expect.stringContaining('Google Gemini (gemini-9.1-flash), com a sua chave'),
    );
    await expect(dialog).toHaveText(expect.stringContaining('No plano gratuito não há custo'));
    await expect(dialog).toHaveText(expect.stringContaining('poucas conversas por dia'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-02-consentimento-gemini');
    await clickStable(dialog.$('button=Começar conversa'));
    await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
    await expect($('p*=com Gemini')).toBeDisplayed();
    const state = await gemini();
    expect(state.refusals).toEqual([]);
    expect(state.generate.length).toBe(2);
  });

  it('CA-T21-06 (ADR-0070): a OpenAI sem chave em primeiro é pulada; a ordem fica depois de reiniciar; nenhuma chave nos arquivos', async () => {
    await openSettings('Sensor Sculk');
    await clickStable(orderRow('openai').$('button[aria-label="Subir OpenAI"]'));
    await browser.waitUntil(() => aiSettings().aiOrder?.[0] === 'openai', {
      timeout: 30_000,
      timeoutMsg: 'a ordem não foi gravada',
    });
    expect(aiSettings()).toMatchObject({
      aiOrder: ['openai', 'gemini', 'anthropic'],
      aiProvider: 'openai',
    });
    await $('.porder__row').waitForDisplayed();
    expect(await $('.porder__row').getAttribute('data-provider')).toBe('openai');
    await expect(orderRow('openai')).toHaveText(expect.stringContaining('Pulado: sem chave'));
    await expect(orderRow('gemini')).toHaveText(expect.stringContaining('Conversas novas'));
    await expect($('nav.sidenav').$('a*=Sensor Sculk')).toHaveText(
      expect.stringContaining('Gemini'),
    );
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-03-sensor-sculk-ordem');

    // Reiniciar mantém a ordem e a chave do Gemini.
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
    await $('header.topbar').waitForDisplayed({ timeout: 60_000 });
    await openSettings('Sensor Sculk');
    await orderRow('gemini').$('span*=Conversas novas').waitForDisplayed({ timeout: 30_000 });
    expect(await $('.porder__row').getAttribute('data-provider')).toBe('openai');

    // Varredura: a chave nunca vai para os arquivos de configuração.
    for (const file of [SETTINGS_FILE, AI_SETTINGS_FILE]) {
      expect(existsSync(file)).toBe(true);
      expect(readFileSync(file, 'utf8')).not.toContain(GEMINI_E2E_KEY);
    }
  });

  it('CA-T14-21 (telas): a conversa do Gemini continua nele; sem a chave dele, pede a chave e nada vai para a rede', async () => {
    // A OpenAI está em primeiro, sem chave: a conversa do Gemini continua nele.
    await openAiSection();
    const list = $('section[aria-labelledby="ia-conversas"]');
    await clickStable(list.$('a*=Continuar'));
    await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
    await expect($('p*=com Gemini')).toBeDisplayed();
    await expect($('aside[aria-label="Sobre esta conversa"]')).toHaveText(
      expect.stringContaining('gemini-9.1-flash'),
    );
    await expect($('div.alert*=Falta a chave do Gemini')).not.toBeExisting();

    // Sem a chave do Gemini, a conversa pede essa chave. Remover avisa que ninguém assume.
    await openSettings('Chaves e contas');
    await clickStable(keyItem('gemini'));
    await clickStable($('section.keydet').$('button=Remover'));
    const confirm = $('[role="alertdialog"]');
    await confirm.waitForDisplayed({ timeout: 10_000 });
    await expect(confirm).toHaveText(
      expect.stringContaining('Nenhum outro provedor tem chave funcionando'),
    );
    await clickStable(confirm.$('button=Remover chave'));
    await keyItem('gemini').$('span*=Sem chave').waitForDisplayed({ timeout: 30_000 });
    const before = (await gemini()).generate.length;

    // Sem nenhuma chave, a seção pede a chave do primeiro da ordem (a OpenAI).
    await openAiSection();
    await $('.empty__title*=Falta a chave da OpenAI').waitForDisplayed({ timeout: 30_000 });
    await clickStable($('section[aria-labelledby="ia-conversas"]').$('a*=Continuar'));
    const notice = $('div.alert*=Falta a chave do Gemini');
    await notice.waitForDisplayed({ timeout: 30_000 });
    await expect(notice).toHaveText(
      expect.stringContaining('Esta conversa começou com Gemini e continua nesse provedor.'),
    );
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-04-conversa-sem-chave');

    // Perguntar de novo: o erro diz qual chave falta, e nenhuma requisição sai.
    await $('textarea').setValue('E se eu tirar o Sodium?');
    await clickStable($('button=Enviar'));
    const failure = $('div[role="alert"]*=Esta conversa começou com Gemini');
    await failure.waitForDisplayed({ timeout: 60_000 });
    await expect(failure).toHaveText(expect.stringContaining('Nada foi alterado no pack.'));
    expect((await gemini()).generate.length).toBe(before);

    // "Abrir Configurações" do aviso leva a Configurações.
    await clickStable(notice.$('a=Abrir Configurações'));
    await waitHeading('Geral');
  });

  /**
   * CA-T14-20 com a OpenAI e o Claude: a chave salva pela tela passa no teste, o provedor vai para
   * o primeiro da ordem, a seção mostra o provedor e o modelo que a regra escolheu na lista do
   * simulado (D-16/D-17), o consentimento diz o provedor e o modelo, sem plano gratuito, e a
   * conversa nova vai só a esse provedor.
   */
  async function conversationWith(options: {
    provider: AiProvider;
    radio: string;
    key: string;
    model: string;
    tier: string;
    charge: string;
    shot: string;
  }): Promise<void> {
    const before = await allProviders();
    await saveKey(options.provider, options.key);
    await moveToTop(options.provider);
    await orderRow(options.provider)
      .$('span*=Conversas novas')
      .waitForDisplayed({ timeout: 30_000 });

    await openAiSection();
    await $('h2=Nova conversa').waitForDisplayed({ timeout: 30_000 });
    const line = $('p.ai-model');
    await browser.waitUntil(
      async () =>
        (await line.getText()).includes(
          `Provedor: ${options.radio}. Modelo: ${options.model} (${options.tier}).`,
        ),
      { timeout: 30_000, timeoutMsg: `a seção não mostrou ${options.model}` },
    );
    await $('select.select').selectByAttribute('value', 'pack');
    await clickStable($('main').$('button=Começar conversa'));
    const dialog = $('[role="dialog"]');
    await dialog.$('h2=Começar uma conversa com a IA?').waitForDisplayed({ timeout: 30_000 });
    await expect(dialog).toHaveText(
      expect.stringContaining(`${options.radio} (${options.model}), com a sua chave`),
    );
    await expect(dialog).toHaveText(expect.stringContaining(options.charge));
    await expect(dialog).not.toHaveText(expect.stringContaining('plano gratuito'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot(options.shot);
    await clickStable(dialog.$('button=Começar conversa'));
    await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
    await expect($(`p*=com ${options.radio}`)).toBeDisplayed();
    await expect($('aside[aria-label="Sobre esta conversa"]')).toHaveText(
      expect.stringContaining(options.model),
    );

    // Só o provedor ativo recebeu a conversa, sem recusa; os outros dois ficaram parados.
    const after = await allProviders();
    for (const provider of AI_PROVIDERS) {
      expect(after[provider].refusals).toEqual([]);
      if (provider === options.provider) {
        expect(after[provider].generate.length).toBeGreaterThan(before[provider].generate.length);
      } else {
        expect(after[provider].generate.length).toBe(before[provider].generate.length);
      }
    }
  }

  it('CA-T14-20 (OpenAI): com a chave da OpenAI, a conversa nova vai só à OpenAI e o consentimento diz "OpenAI (gpt-…)"', async () => {
    // O modelo da conversa é o "Mais econômico" (o modo de cada pergunta é da T1).
    await conversationWith({
      provider: 'openai',
      radio: 'OpenAI',
      key: OPENAI_E2E_KEY,
      model: OPENAI_E2E_MODEL,
      tier: 'mais econômico',
      charge: 'O uso é cobrado na sua chave da OpenAI',
      shot: 'd18-05-consentimento-openai',
    });
  });

  it('CA-T14-20 (Claude): com a chave do Claude, a conversa nova vai só à Anthropic e o consentimento diz "Claude (claude-…)"', async () => {
    // O Claude ficou em "Mais econômico": a regra dá o Sonnet mais novo da lista.
    await conversationWith({
      provider: 'anthropic',
      radio: 'Claude',
      key: ANTHROPIC_E2E_KEY,
      model: ANTHROPIC_E2E_MODEL,
      tier: 'mais econômico',
      charge: 'O uso é cobrado na sua chave da Anthropic',
      shot: 'd18-06-consentimento-claude',
    });
  });
});
