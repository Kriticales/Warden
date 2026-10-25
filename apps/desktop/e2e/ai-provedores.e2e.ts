/**
 * O Sensor Sculk com vários provedores no app real (D-18; ADR-0060 e ADR-0064; SPEC T14 e T21),
 * com os servidores simulados de `mock-server/` (o do Gemini exige a chave só no cabeçalho e o
 * histórico intacto, como a API real):
 *
 * - CA-T21-06: em Chaves e contas › Sensor Sculk, o Gemini vem como padrão; a chave salva pela
 *   tela passa no Testar; a linha única do Gemini mostra o modelo; a escolha do provedor fica ao
 *   reiniciar; não há mais escolha de modelo (CA-T14-31: um `ai/settings.json` antigo, com a
 *   escolha fixa, continua abrindo e a perde na próxima gravação); nenhuma chave aparece em
 *   `settings.json` nem em `ai/settings.json` (varredura);
 * - CA-T14-20 (Gemini): a conversa nova usa o provedor escolhido; o consentimento mostra o nome
 *   do provedor e o modelo, com o aviso de plano gratuito;
 * - CA-T14-21 (telas): trocar o provedor ativo não muda a conversa já começada, que continua
 *   abrindo com o Gemini; sem a chave do Gemini, ela pede essa chave e leva a Configurações, e
 *   perguntar de novo mostra qual chave falta, sem nenhuma requisição de rede.
 *
 * - CA-T14-20 (OpenAI e Claude, ligados na junção do lote 12 com os simulados e os tradutores da
 *   D-16/D-17): a chave pela tela, o modelo da regra na Nova conversa e no consentimento, sem
 *   plano gratuito, e a conversa nova só no provedor ativo;
 * - CA-T14-31: no Claude, "Procurar rápido" manda a primeira pergunta no Sonnet e "Procurar a
 *   fundo" manda a pergunta seguinte no Opus; Detalhes mostra os dois modos.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { ANTHROPIC_E2E_KEY, ANTHROPIC_E2E_MODEL } from './mock-server/anthropic';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
import { OPENAI_E2E_KEY } from './mock-server/openai';
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

/** A área do Sensor Sculk em Chaves e contas. */
const aiArea = () => $('section[aria-labelledby]*=Provedor do Sculk');
const providerRadio = (name: string) =>
  $('fieldset*=Provedor do Sculk').$(`label.choice*=${name}`).$('input[type="radio"]');
const keyRow = (kind: string) => $(`[data-testid="chave-${kind}"]`);

/** Meus packs, de onde estiver (a volta do pack ou de Configurações). */
async function openMyPacks(): Promise<void> {
  if (!(await $('h1=Meus packs').isDisplayed())) {
    await clickStable($('a*=Meus packs'));
  }
  await waitHeading('Meus packs');
}

async function openSettings(): Promise<void> {
  await openMyPacks();
  await clickStable($('header.topbar').$('a*=Configurações'));
  await waitHeading('Configurações');
  await aiArea().waitForDisplayed({ timeout: 30_000 });
}

/** Meus packs → o pack → Sensor Sculk. */
async function openAiSection(): Promise<void> {
  await openMyPacks();
  await clickStable($(`tr.packrow*=${PACK_NAME}`).$('button.packrow__title'));
  await waitHeading('Mods');
  await clickStable($('nav[aria-label="Seções do pack"]').$('a*=Sensor Sculk'));
  await $('aside.sculk__convs').waitForDisplayed({ timeout: 30_000 });
}

/** Nova conversa: só o pack, "Outra coisa" e Começar conversa; devolve o consentimento. */
async function startNew(): Promise<ReturnType<typeof $>> {
  const form = $('form.newconv');
  await form.waitForDisplayed({ timeout: 30_000 });
  await form.$('select.select').selectByAttribute('value', 'pack');
  await clickStable(form.$('button[role="radio"]*=Outra coisa'));
  await clickStable(form.$('button*=Começar conversa'));
  const dialog = $('[role="dialog"]');
  await dialog.$('h2*=Começar uma conversa com o Sculk?').waitForDisplayed({ timeout: 30_000 });
  return dialog;
}

/** Espera a resposta do Sculk (o rodapé "O Sculk pode errar…"). */
async function waitAnswer(count = 1): Promise<void> {
  await browser.waitUntil(async () => (await $$('.ai__foot').length) >= count, {
    timeout: 60_000,
    timeoutMsg: 'a resposta do Sculk não chegou',
  });
}

/** Abre a gaveta Detalhes da conversa aberta. */
async function openDetails(): Promise<ReturnType<typeof $>> {
  await clickStable($('button[aria-controls="sculk-detalhes"]'));
  const drawer = $('#sculk-detalhes');
  await drawer.waitForDisplayed({ timeout: 10_000 });
  return drawer;
}

/** Escolhe um rádio e espera a escolha valer (o backend grava e a tela relê). */
async function choose(radio: ReturnType<typeof $>): Promise<void> {
  await clickStable(radio.parentElement().parentElement());
  await browser.waitUntil(async () => radio.isSelected(), {
    timeout: 30_000,
    timeoutMsg: 'a escolha não ficou marcada',
  });
}

function aiSettings(): Record<string, unknown> {
  return JSON.parse(readFileSync(AI_SETTINGS_FILE, 'utf8')) as Record<string, unknown>;
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

    await openSettings();
    expect(await providerRadio('Gemini').isSelected()).toBe(true);
    const row = keyRow('gemini');
    await row.$('input[aria-label="Chave do Gemini"]').setValue(GEMINI_E2E_KEY);
    await clickStable(row.$('button=Salvar'));
    await row.$('span*=Configurada').waitForDisplayed({ timeout: 30_000 });
    await clickStable(row.$('button=Testar'));
    await row.$('span*=Funcionando').waitForDisplayed({ timeout: 30_000 });
    const line = $('[data-testid="ia-modelo-gemini"]');
    await expect(line).toHaveText(
      expect.stringContaining('Usa sempre o modelo mais forte do plano gratuito'),
    );
    await line.$('span*=Hoje: gemini-9.1-flash.').waitForDisplayed({ timeout: 30_000 });
    await expect(line).toHaveText(
      expect.stringContaining('para o mais leve (gemini-9.1-flash-lite), só naquela pergunta'),
    );
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-01-configuracoes-gemini');

    await openAiSection();
    // No Gemini não há "Procurar rápido | Procurar a fundo".
    await expect($('form.newconv').$('span=Como procurar')).not.toBeExisting();
    const dialog = await startNew();
    await expect(dialog).toHaveText(
      expect.stringContaining('Google Gemini (gemini-9.1-flash), com a sua chave'),
    );
    await expect(dialog).toHaveText(expect.stringContaining('No plano gratuito não há custo'));
    await expect(dialog).toHaveText(expect.stringContaining('poucas conversas por dia'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-02-consentimento-gemini');
    await clickStable(dialog.$('button*=Enviar e começar'));
    await waitAnswer();
    await expect($('span.sculk__meta*=Gemini ·')).toBeDisplayed();
    const state = await gemini();
    expect(state.refusals).toEqual([]);
    expect(state.generate.length).toBe(2);
  });

  it('CA-T21-06 e CA-T14-31: o provedor fica depois de reiniciar; a escolha fixa do modelo some; nenhuma chave nos arquivos', async () => {
    // Um `ai/settings.json` de antes da ADR-0064, com a escolha fixa do modelo: continua abrindo.
    writeFileSync(
      AI_SETTINGS_FILE,
      '{ "aiProvider": "gemini", "tier": "careful", "tiers": { "openai": "careful" } }\n',
    );
    await openSettings();
    expect(await providerRadio('Gemini').isSelected()).toBe(true);
    await choose(providerRadio('OpenAI'));
    const openai = $('div[data-testid="ia-modelos"]');
    await openai.waitForDisplayed({ timeout: 30_000 });
    await expect(openai).toHaveText(expect.stringContaining('A OpenAI cobra por uso'));
    await expect(openai).toHaveText(
      expect.stringContaining('Procurar rápido ou Procurar a fundo a cada pergunta'),
    );
    await expect($('label.choice*=Análise mais cuidadosa')).not.toBeExisting();
    expect(aiSettings()).toEqual({ aiProvider: 'openai' });
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-03-configuracoes-openai');

    // Reiniciar mantém o provedor e a chave do Gemini.
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
    await $('header.topbar').waitForDisplayed({ timeout: 60_000 });
    await openSettings();
    await browser.waitUntil(async () => providerRadio('OpenAI').isSelected(), {
      timeout: 30_000,
      timeoutMsg: 'a OpenAI não continuou escolhida depois de reiniciar',
    });
    await keyRow('gemini').$('span*=Configurada').waitForDisplayed({ timeout: 30_000 });

    // Varredura: a chave nunca vai para os arquivos de configuração.
    for (const file of [SETTINGS_FILE, AI_SETTINGS_FILE]) {
      expect(existsSync(file)).toBe(true);
      expect(readFileSync(file, 'utf8')).not.toContain(GEMINI_E2E_KEY);
    }
  });

  it('CA-T14-21 (telas): a conversa do Gemini continua nele; sem a chave dele, pede a chave e nada vai para a rede', async () => {
    // O provedor ativo é a OpenAI, sem chave: a seção pede a chave dela, e a conversa continua lá.
    await openAiSection();
    await $('.empty__title*=Falta a chave da OpenAI').waitForDisplayed({ timeout: 30_000 });
    await clickStable($('aside.sculk__convs').$('a.conv'));
    await waitAnswer();
    await expect($('span.sculk__meta*=Gemini ·')).toBeDisplayed();
    const details = await openDetails();
    await expect(details).toHaveText(expect.stringContaining('gemini-9.1-flash'));
    await browser.keys('Escape');
    await details.waitForExist({ reverse: true, timeout: 10_000 });
    await expect($('div.alert*=Falta a chave do Gemini')).not.toBeExisting();

    // Sem a chave do Gemini, a conversa pede essa chave.
    await openSettings();
    await clickStable(keyRow('gemini').$('button=Remover'));
    const confirm = $('[role="alertdialog"]');
    await confirm.waitForDisplayed({ timeout: 10_000 });
    await clickStable(confirm.$('button=Remover chave'));
    await keyRow('gemini').$('input[aria-label="Chave do Gemini"]').waitForDisplayed({
      timeout: 30_000,
    });
    const before = (await gemini()).generate.length;

    await openAiSection();
    await clickStable($('aside.sculk__convs').$('a.conv'));
    const notice = $('div.alert*=Falta a chave do Gemini');
    await notice.waitForDisplayed({ timeout: 30_000 });
    await expect(notice).toHaveText(
      expect.stringContaining('Esta conversa começou com Gemini e continua nesse provedor.'),
    );
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-04-conversa-sem-chave');

    // Perguntar de novo: o erro diz qual chave falta, e nenhuma requisição sai.
    await $('form.composer').$('textarea').setValue('E se eu tirar o Sodium?');
    await clickStable($('form.composer').$('button*=Enviar'));
    const failure = $('div[role="alert"]*=Esta conversa começou com Gemini');
    await failure.waitForDisplayed({ timeout: 60_000 });
    await expect(failure).toHaveText(expect.stringContaining('Nada foi alterado no pack.'));
    expect((await gemini()).generate.length).toBe(before);

    // "Abrir Configurações" do aviso leva a Chaves e contas.
    await clickStable(notice.$('a=Abrir Configurações'));
    await waitHeading('Configurações');
    await aiArea().waitForDisplayed({ timeout: 30_000 });
  });

  /**
   * CA-T14-20 com a OpenAI e o Claude: a chave salva pela tela passa no Testar, a Nova conversa
   * mostra o provedor e o modelo que a regra escolheu na lista do simulado para o modo (D-16/D-17;
   * ADR-0064), o consentimento diz o provedor e o modelo, sem plano gratuito, e a conversa nova
   * vai só ao provedor ativo.
   */
  async function conversationWith(options: {
    provider: AiProvider;
    radio: string;
    key: string;
    model: string;
    mode: 'Procurar rápido' | 'Procurar a fundo';
    charge: string;
    shot: string;
  }): Promise<void> {
    const before = await allProviders();
    await openSettings();
    await choose(providerRadio(options.radio));
    const row = keyRow(options.provider);
    await row.$(`input[aria-label="Chave ${keyLabel(options.provider)}"]`).setValue(options.key);
    await clickStable(row.$('button=Salvar'));
    await row.$('span*=Configurada').waitForDisplayed({ timeout: 30_000 });
    await clickStable(row.$('button=Testar'));
    await row.$('span*=Funcionando').waitForDisplayed({ timeout: 30_000 });

    await openAiSection();
    await clickStable($('aside.sculk__convs').$('button*=Nova conversa'));
    const form = $('form.newconv');
    await form.waitForDisplayed({ timeout: 30_000 });
    await clickStable(form.$(`button[role="radio"]=${options.mode}`));
    const line = form.$('p.provline');
    await browser.waitUntil(
      async () =>
        (await line.getText()).includes(`Provedor: ${options.radio}. Modelo: ${options.model}.`),
      { timeout: 30_000, timeoutMsg: `a Nova conversa não mostrou ${options.model}` },
    );
    const dialog = await startNew();
    await expect(dialog).toHaveText(
      expect.stringContaining(`${options.radio} (${options.model}), com a sua chave`),
    );
    await expect(dialog).toHaveText(expect.stringContaining(options.charge));
    await expect(dialog).not.toHaveText(expect.stringContaining('plano gratuito'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot(options.shot);
    await clickStable(dialog.$('button*=Enviar e começar'));
    await waitAnswer();
    await expect($(`span.sculk__meta*=${options.radio} ·`)).toBeDisplayed();
    const details = await openDetails();
    await expect(details).toHaveText(expect.stringContaining(options.model));
    await expect(details).toHaveText(expect.stringContaining(options.mode));
    await browser.keys('Escape');
    await details.waitForExist({ reverse: true, timeout: 10_000 });

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
    // "Procurar a fundo": a regra dá o `gpt-N-sol`.
    await conversationWith({
      provider: 'openai',
      radio: 'OpenAI',
      key: OPENAI_E2E_KEY,
      model: 'gpt-9-sol',
      mode: 'Procurar a fundo',
      charge: 'O uso é cobrado na sua chave da OpenAI',
      shot: 'd18-05-consentimento-openai',
    });
  });

  it('CA-T14-20 e CA-T14-31 (Claude): a conversa nova vai só à Anthropic; cada pergunta usa o modelo do seu modo', async () => {
    // "Procurar rápido": a regra dá o Sonnet mais novo da lista.
    await conversationWith({
      provider: 'anthropic',
      radio: 'Claude',
      key: ANTHROPIC_E2E_KEY,
      model: ANTHROPIC_E2E_MODEL,
      mode: 'Procurar rápido',
      charge: 'O uso é cobrado na sua chave da Anthropic',
      shot: 'd18-06-consentimento-claude',
    });
    const anthropic = async () =>
      ((await (await fetch(`${MOCK_URL}/__anthropic`)).json()) as GeminiState).generate;
    const before = (await anthropic()).length;
    expect(
      (await anthropic()).every(
        (body) => (JSON.parse(body) as { model: string }).model === ANTHROPIC_E2E_MODEL,
      ),
    ).toBe(true);

    // A pergunta seguinte, "Procurar a fundo", sai no Opus mais novo da lista.
    const composer = $('form.composer');
    await expect(
      composer.$('[role="radiogroup"][aria-label="Como o Sculk procura"]'),
    ).toBeDisplayed();
    await clickStable(composer.$('button[role="radio"]=Procurar a fundo'));
    await composer.$('textarea').setValue('E se eu tirar o Sodium?');
    await clickStable(composer.$('button*=Enviar'));
    await waitAnswer(2);
    const after = await anthropic();
    expect(after.length).toBeGreaterThan(before);
    for (const body of after.slice(before)) {
      expect((JSON.parse(body) as { model: string }).model).toBe('claude-opus-9-5');
    }
    const details = await openDetails();
    await expect(details).toHaveText(expect.stringContaining('1 rápido, 1 a fundo'));
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-07-conversa-claude-modos');
  });
});
