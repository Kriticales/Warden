/**
 * IA com vários provedores no app real (D-18; ADR-0060; SPEC T14 ✦ e T21), com os servidores
 * simulados de `mock-server/` (o do Gemini exige a chave só no cabeçalho e o histórico intacto,
 * como a API real):
 *
 * - CA-T21-06: em Chaves e contas › IA, o Gemini vem como padrão; a chave salva pela tela passa
 *   no Testar; a linha única do Gemini mostra o modelo; trocar para a OpenAI, escolher "Análise
 *   mais cuidadosa", ir ao Claude e voltar mantém a escolha de cada um; reiniciar mantém tudo;
 *   nenhuma chave aparece em `settings.json` nem em `ai/settings.json` (varredura);
 * - CA-T14-20 (Gemini): a conversa nova usa o provedor escolhido; o consentimento mostra o nome
 *   do provedor e o modelo, com o aviso de plano gratuito;
 * - CA-T14-21 (telas): trocar o provedor ativo não muda a conversa já começada, que continua
 *   abrindo com o Gemini; sem a chave do Gemini, ela pede essa chave e leva a Configurações, e
 *   perguntar de novo mostra qual chave falta, sem nenhuma requisição de rede.
 *
 * Os casos da OpenAI e do Claude (CA-T14-20 com os simulados deles) ligam na junção com a
 * D-16/D-17, que trazem `mock-server/openai.ts` e `mock-server/anthropic.ts` e os tradutores.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
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

/** A área IA de Chaves e contas. */
const aiArea = () => $('section[aria-labelledby]*=Provedor da IA');
const providerRadio = (name: string) =>
  $('fieldset*=Provedor da IA').$(`label.choice*=${name}`).$('input[type="radio"]');
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

/** Meus packs → o pack → ✦ Diagnóstico com IA. */
async function openAiSection(): Promise<void> {
  await openMyPacks();
  await clickStable($(`tr.packrow*=${PACK_NAME}`).$('button.packrow__title'));
  await waitHeading('Mods');
  await clickStable($('nav[aria-label="Seções do pack"]').$('a*=Diagnóstico com IA'));
  await waitHeading('Diagnóstico com IA');
}

/** Escolhe um rádio e espera a escolha valer (o backend grava e a tela relê). */
async function choose(radio: ReturnType<typeof $>): Promise<void> {
  await clickStable(radio.parentElement().parentElement());
  await browser.waitUntil(async () => radio.isSelected(), {
    timeout: 30_000,
    timeoutMsg: 'a escolha não ficou marcada',
  });
}

function aiSettings(): { aiProvider?: string; tiers?: Record<string, string> } {
  return JSON.parse(readFileSync(AI_SETTINGS_FILE, 'utf8')) as {
    aiProvider?: string;
    tiers?: Record<string, string>;
  };
}

describe('IA com vários provedores (D-18)', function () {
  before(async function () {
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(KEY_FILE, { force: true });
    rmSync(AI_SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await fetch(`${MOCK_URL}/__gemini/reset`, { method: 'POST' });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
  });

  after(() => {
    rmSync(KEY_FILE, { force: true });
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

  it('CA-T21-06: a escolha de cada provedor fica ao trocar e voltar, e depois de reiniciar; nenhuma chave nos arquivos', async () => {
    await openSettings();
    await choose(providerRadio('OpenAI'));
    const openai = $('fieldset*=Modelo (OpenAI)');
    await openai.waitForDisplayed({ timeout: 30_000 });
    await expect(openai).toHaveText(expect.stringContaining('A OpenAI cobra por uso'));
    await choose(openai.$('label.choice*=Análise mais cuidadosa').$('input'));
    await choose(providerRadio('Claude'));
    const claude = $('fieldset*=Modelo (Claude)');
    await claude.waitForDisplayed({ timeout: 30_000 });
    expect(await claude.$('label.choice*=Mais econômico').$('input').isSelected()).toBe(true);
    await choose(providerRadio('OpenAI'));
    await openai.waitForDisplayed({ timeout: 30_000 });
    expect(await openai.$('label.choice*=Análise mais cuidadosa').$('input').isSelected()).toBe(
      true,
    );
    expect(aiSettings()).toMatchObject({ aiProvider: 'openai', tiers: { openai: 'careful' } });
    expect(await axeAfterToasts()).toEqual([]);
    await screenshot('d18-03-configuracoes-openai');

    // Reiniciar mantém o provedor, a escolha e a chave do Gemini.
    await browser.reloadSession();
    await browser.setWindowSize(1366, 900);
    await $('header.topbar').waitForDisplayed({ timeout: 60_000 });
    await openSettings();
    await browser.waitUntil(async () => providerRadio('OpenAI').isSelected(), {
      timeout: 30_000,
      timeoutMsg: 'a OpenAI não continuou escolhida depois de reiniciar',
    });
    expect(
      await $('fieldset*=Modelo (OpenAI)')
        .$('label.choice*=Análise mais cuidadosa')
        .$('input')
        .isSelected(),
    ).toBe(true);
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
    const list = $('section[aria-labelledby="ia-conversas"]');
    await clickStable(list.$('a*=Continuar'));
    await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
    await expect($('p*=com Gemini')).toBeDisplayed();
    await expect($('aside[aria-label="Sobre esta conversa"]')).toHaveText(
      expect.stringContaining('gemini-9.1-flash'),
    );
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

    // "Abrir Configurações" do aviso leva a Chaves e contas.
    await clickStable(notice.$('a=Abrir Configurações'));
    await waitHeading('Configurações');
    await aiArea().waitForDisplayed({ timeout: 30_000 });
  });

  // Liga na junção com a D-16/D-17: os simulados da OpenAI e da Anthropic
  // (`mock-server/openai.ts`, `mock-server/anthropic.ts`) e os tradutores ainda não estão nesta base.
  it.skip('CA-T14-20 (OpenAI): com a chave da OpenAI, a conversa nova vai só à OpenAI e o consentimento diz "OpenAI (gpt-…)"', async () => {
    // Na junção: gravar a chave da OpenAI pela tela (keyRow('openai')), escolher a OpenAI,
    // abrir o consentimento (texto "OpenAI (<modelo>), com a sua chave" e "O uso é cobrado na sua
    // chave da OpenAI", sem "plano gratuito"), começar, esperar "O que a IA afirma" e conferir no
    // simulado da OpenAI os corpos recebidos e, no do Gemini, nenhum `generateContent` a mais.
  });

  // Liga na junção com a D-16/D-17 (simulado e tradutor da Anthropic).
  it.skip('CA-T14-20 (Claude): com a chave do Claude, a conversa nova vai só à Anthropic e o consentimento diz "Claude (Anthropic) (claude-…)"', async () => {
    // Na junção: o mesmo roteiro do caso da OpenAI, com keyRow('anthropic'), o rádio "Claude" e o
    // simulado da Anthropic; nenhuma requisição à OpenAI nem ao Gemini.
  });
});
