/**
 * CurseForge em Adicionar no app real (P1-10; SPEC T08 e T09), com o packwiz de verdade e a
 * CurseForge e o Modrinth simulados (servidor de fixtures; nada vai à internet).
 *
 * O pack é o que o packwiz real gerou para o Fabric 1.21.1
 * (`crates/warden-packwiz/tests/fixtures/packwiz-output/fabric-1.21.1`), sem o Sodium.
 *
 * - Sem chave: só o Modrinth, com o aviso, e nenhuma requisição à CurseForge (CA-T08-06).
 * - Com chave: o Sodium nas duas fontes aparece uma vez ("Modrinth e CurseForge"), o mod só da
 *   CurseForge vem marcado "CurseForge" e o bloqueado "Download manual necessário" (CA-T08-08).
 * - A pré-visualização confere o SHA-1; escolher a CurseForge grava `mode = "metadata:curseforge"`.
 * - Um mod da CurseForge entra com a dependência obrigatória dele, numa gravação só.
 * - Chave recusada: aviso certo e a CurseForge não é mais consultada; fonte com erro: aviso com
 *   "Tentar de novo" e os resultados do Modrinth (CA-T08-09).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { CHAVE_BOA, CHAVE_COM_ERRO, CHAVE_RECUSADA } from './mock-server/curseforge';
import { dismissToasts } from './toasts';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');
const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const FIXTURE = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  'crates',
  'warden-packwiz',
  'tests',
  'fixtures',
  'packwiz-output',
  'fabric-1.21.1',
);
const PACK = join(DATA_ROOT, 'fora', 'pack-curseforge');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
/** Onde o cofre de teste (arquivo) guarda a chave da CurseForge (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'curseforge-api-key');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function pickNext(folder: string): void {
  writeFileSync(PICK_FILE, folder);
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

function setKey(key: string | null): void {
  if (key === null) {
    rmSync(KEY_FILE, { force: true });
    return;
  }
  mkdirSync(join(DATA_ROOT, 'cofre-de-teste'), { recursive: true });
  writeFileSync(KEY_FILE, key);
}

/** Tira o Sodium do pack copiado, como se ele nunca tivesse entrado (índice e hash dele). */
function removeSodium(): void {
  rmSync(join(PACK, 'mods', 'sodium.pw.toml'));
  const index = read('index.toml').replace(
    /\[\[files\]\]\nfile = "mods\/sodium\.pw\.toml"\nhash = "[0-9a-f]+"\nmetafile = true\n\n/,
    '',
  );
  expect(index).not.toContain('sodium');
  writeFileSync(join(PACK, 'index.toml'), index);
  const hash = createHash('sha256').update(index).digest('hex');
  writeFileSync(
    join(PACK, 'pack.toml'),
    read('pack.toml').replace(/^hash = "[0-9a-f]+"$/m, `hash = "${hash}"`),
  );
}

interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
}

async function seriousViolations(): Promise<string[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async () => {
    const { axe } = window as unknown as {
      axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
    };
    try {
      return (await axe.run({ resultTypes: ['violations'] })).violations;
    } catch {
      const failed: AxeViolation = {
        id: 'axe-falhou',
        impact: 'critical',
        help: 'o axe não rodou',
      };
      return [failed];
    }
  });
  return violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id}: ${v.help}`);
}

/** Espera um aviso (toast) com o texto; um aviso de erro falha na hora, com a frase. */
async function waitToast(text: string): Promise<void> {
  let seen = '';
  await browser.waitUntil(
    async () => {
      seen = await browser.execute(() =>
        Array.from(document.querySelectorAll('.toast'))
          .map((toast) => toast.textContent)
          .join(' | '),
      );
      return seen.includes(text) || seen.includes('Não foi possível') || /erro/i.test(seen);
    },
    { timeout: 45_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
  expect(seen).toContain(text);
}

/** Texto completo de um elemento (inclusive opções de seletores e o que está fora da tela). */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** As requisições que o servidor de fixtures recebeu até agora. */
async function mockRequests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

async function curseforgeRequests(): Promise<string[]> {
  return (await mockRequests()).filter((request) => request.includes(' /curseforge/'));
}

async function resetRequests(): Promise<void> {
  await fetch(`${MOCK_URL}/__reset`, { method: 'POST' });
}

/** Busca um texto novo (outra consulta, para não vir do cache de 5 minutos da tela). */
async function searchFor(text: string): Promise<void> {
  const field = $('input[type="search"]');
  await field.setValue(text);
  await browser.waitUntil(async () => (await field.getValue()) === text);
}

/** A linha de resultado com este nome. */
function row(name: string) {
  return $(
    `.//li[contains(@class,"drow")][.//button[contains(@class,"drow__name")][normalize-space()="${name}"]]`,
  );
}

/** Quantas vezes o nome aparece na lista. */
async function countResults(name: string): Promise<number> {
  return browser.execute(
    (wanted) =>
      Array.from(document.querySelectorAll('.drow .drow__name')).filter(
        (b) => b.textContent === wanted,
      ).length,
    name,
  );
}

describe('Adicionar pela CurseForge (P1-10)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    removeSodium();
    pickNext('');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    setKey(null);
    await resetRequests();
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados como estava para os testes que rodam depois.
    setKey(null);
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
  });

  it('abre o pack e a página Adicionar', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
    await $('a=Adicionar').click();
    await $('h1=Adicionar ao pack').waitForDisplayed();
  });

  it('CA-T08-06: sem chave, só o Modrinth, com o aviso e nenhuma requisição à CurseForge', async () => {
    await resetRequests();
    await searchFor('sodium');
    await $('button.drow__name=Sodium').waitForDisplayed({ timeout: 30_000 });
    await $('div.alert*=Mostrando só o Modrinth.').waitForDisplayed();
    const body = await $('body').getText();
    expect(body).toContain('Para buscar também na CurseForge, informe sua chave em Configurações.');
    await expect($('a=Abrir Configurações')).toBeDisplayed();
    expect(await row('Sodium').getText()).not.toContain('CurseForge');
    expect(await curseforgeRequests()).toEqual([]);
    expect(await seriousViolations()).toEqual([]);
    await screenshot('01-sem-chave');
  });

  it('CA-T08-08: com chave, o Sodium das duas fontes aparece uma vez; o resto vem marcado', async () => {
    setKey(CHAVE_BOA);
    await searchFor('sodium mod');
    await $('.drow*=Mod Só da CurseForge').waitForDisplayed({ timeout: 30_000 });
    expect(await $('body').getText()).not.toContain('Mostrando só o Modrinth.');
    expect(await countResults('Sodium')).toBe(1);
    expect(await row('Sodium').getText()).toContain('Modrinth e CurseForge');
    const only = await row('Mod Só da CurseForge').getText();
    expect(only).toContain('CurseForge');
    expect(only).not.toContain('Modrinth');
    expect(await row('Mod Bloqueado').getText()).toContain('Download manual necessário');
    // A chave do usuário foi a que a CurseForge recebeu.
    expect((await curseforgeRequests()).length).toBeGreaterThan(0);
    expect(await seriousViolations()).toEqual([]);
    await screenshot('02-com-chave');
  });

  it('a pré-visualização confere o SHA-1; escolher a CurseForge grava mode = "metadata:curseforge"', async () => {
    await $('button.drow__name=Sodium').click();
    const preview = $('aside.disc__preview');
    await preview.waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(
      async () => (await fullText('aside.disc__preview')).includes('o SHA-1 confere'),
      { timeout: 30_000, timeoutMsg: 'a conferência do SHA-1 não apareceu' },
    );
    expect(await fullText('aside.disc__preview')).toContain('Modrinth (recomendada)');
    await screenshot('03-previa-duas-fontes');

    await $$('aside.disc__preview select')[0]?.selectByVisibleText('CurseForge');
    await browser.waitUntil(
      async () =>
        (await fullText('aside.disc__preview')).includes('sodium-fabric-0.6.0+mc1.21.1.jar') ||
        (await fullText('aside.disc__preview')).includes('(mais nova compatível)'),
      { timeout: 30_000 },
    );
    await dismissToasts();
    await preview.$('button=Adicionar ao pack').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('O que você escolheu'),
      { timeout: 30_000, timeoutMsg: 'o plano não apareceu no diálogo' },
    );
    expect(await fullText('[role="dialog"]')).toContain(
      'Lado desconhecido — confira se é só de cliente.',
    );
    await screenshot('04-dependencias-curseforge');
    await dialog.$('button=Adicionar 1 item').click();
    await waitToast('1 item adicionado ao pack');
    await dialog.waitForExist({ reverse: true });

    const sodium = read('mods/sodium.pw.toml');
    expect(sodium).toContain('mode = "metadata:curseforge"');
    expect(sodium).toContain('hash-format = "sha1"');
    expect(sodium).toContain('hash = "003c114c85ca88ef3362e018deb6aca0c682d6a1"');
    expect(sodium).toContain('project-id = 394468');
    expect(sodium).toContain('side = "both"');
    expect(sodium).not.toContain('[update.modrinth]');
    expect(read('index.toml')).toContain('mods/sodium.pw.toml');
    await row('Sodium').$('.tag*=Já no pack').waitForDisplayed({ timeout: 30_000 });
  });

  it('um mod só da CurseForge entra com a dependência obrigatória, numa gravação só', async () => {
    await dismissToasts();
    await $('input[aria-label="Selecionar Mod Só da CurseForge"]').click();
    await $('button=Adicionar 1 ao pack').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('Biblioteca da CurseForge'),
      { timeout: 30_000, timeoutMsg: 'a dependência não apareceu no plano' },
    );
    expect(await fullText('[role="dialog"]')).toContain('Obrigatórias');
    await screenshot('05-dependencia-curseforge');
    await dialog.$('button=Adicionar 2 itens').click();
    await waitToast('2 itens adicionados ao pack');
    await dialog.waitForExist({ reverse: true });
    expect(read('mods/mod-so-da-curseforge.pw.toml')).toContain('mode = "metadata:curseforge"');
    expect(read('mods/biblioteca-curseforge.pw.toml')).toContain('metadata:curseforge');
    const index = read('index.toml');
    expect(index).toContain('mods/mod-so-da-curseforge.pw.toml');
    expect(index).toContain('mods/biblioteca-curseforge.pw.toml');
    expect(await $('body').getText()).toContain('Voltar para Mods · 3 adicionados');
  });

  it('chave recusada: aviso certo e a CurseForge não é mais consultada', async () => {
    await dismissToasts();
    setKey(CHAVE_RECUSADA);
    await searchFor('sodium recusada');
    await $('div.alert*=A CurseForge recusou a chave.').waitForDisplayed({ timeout: 30_000 });
    expect(await $('body').getText()).toContain(
      'Confira em Configurações. Mostrando só o Modrinth.',
    );
    await $('button.drow__name=Sodium Extra').waitForDisplayed({ timeout: 30_000 });
    await screenshot('06-chave-recusada');
    // A segunda busca com a mesma chave nem pergunta à CurseForge.
    await resetRequests();
    await searchFor('sodium de novo');
    await $('div.alert*=A CurseForge recusou a chave.').waitForDisplayed({ timeout: 30_000 });
    await $('button.drow__name=Sodium Extra').waitForDisplayed({ timeout: 30_000 });
    expect(await curseforgeRequests()).toEqual([]);
  });

  it('CA-T08-09: CurseForge com erro 500 mostra o Modrinth e o aviso com "Tentar de novo"', async () => {
    setKey(CHAVE_COM_ERRO);
    await searchFor('sodium com erro');
    await $('div.alert*=Não foi possível buscar no CurseForge agora.').waitForDisplayed({
      timeout: 60_000,
    });
    await $('button.drow__name=Sodium Extra').waitForDisplayed({ timeout: 30_000 });
    await expect($('button=Tentar de novo')).toBeDisplayed();
    await screenshot('07-fonte-com-erro');
    expect(await seriousViolations()).toEqual([]);
  });

  it('voltar para Mods mostra os itens da CurseForge na lista', async () => {
    setKey(CHAVE_BOA);
    await $('a*=Voltar para Mods').click();
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Sodium').waitForDisplayed({ timeout: 30_000 });
    await $('tr.modrow*=Biblioteca da CurseForge').waitForDisplayed({ timeout: 30_000 });
  });
});
