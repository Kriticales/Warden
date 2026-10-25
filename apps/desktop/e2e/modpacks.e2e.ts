/**
 * Navegar por modpacks no app real (P1-17; SPEC T08, CA-T08-14, CA-T08-15 e CA-T08-16), com o
 * packwiz de verdade e o servidor de fixtures (nada vai à internet):
 *
 * - Tipo = Modpacks busca modpacks; abrir o "Pacote Leve" do Modrinth lista os mods com no
 *   máximo 4 requisições ao Modrinth e marca "No pack", compatível, "Sem versão para…" e
 *   "Arquivo fora das lojas".
 * - "Adicionar 2 selecionados" abre um diálogo de dependências só, grava com a versão do
 *   modpack e a lista passa a mostrar os dois como "No pack".
 * - Com a chave da CurseForge, o "Vale CurseForge" abre pelo `manifest.json` lido por partes: o
 *   app resolve o redirecionamento do "edge" antes (que responde 404 a pedidos com `Range`) e só
 *   pede partes ao servidor final. Depois da sessão, nada do manifesto nem da resposta da API
 *   está na pasta de dados.
 */
import {
  cpSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { CHAVE_BOA } from './mock-server/curseforge';
import { API_MARKER, MANIFEST_MARKER } from './mock-server/modpacks';
import { dismissToasts } from './toasts';
import { openOrImport } from './open-or-import';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-modpacks');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'curseforge-api-key');
const PANEL_LEVE = 'aside[aria-label="Mods do modpack Pacote Leve"]';
const PANEL_VALE = 'aside[aria-label="Mods do modpack Vale CurseForge"]';

async function mockRequests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

async function resetRequests(): Promise<void> {
  await fetch(`${MOCK_URL}/__reset`, { method: 'POST' });
}

async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** O texto da linha de um mod no painel do modpack. */
async function modRow(panel: string, name: string): Promise<string> {
  return browser.execute(
    (sel, wanted) =>
      Array.from(document.querySelectorAll(`${sel} li.drow`)).find(
        (row) => row.querySelector('.modpack__name')?.textContent === wanted,
      )?.textContent ?? '',
    panel,
    name,
  );
}

/** Clica num elemento depois de fechar os avisos e de o elemento parar no lugar. */
async function steadyClick(selector: string): Promise<void> {
  await dismissToasts();
  const element = $(selector);
  await element.waitForDisplayed({ timeout: 30_000 });
  let last = '';
  await browser.waitUntil(
    async () => {
      const now = await browser.execute(
        (node) => {
          node.scrollIntoView({ block: 'center' });
          const rect = node.getBoundingClientRect();
          return `${String(rect.x)},${String(rect.y)}`;
        },
        await element.getElement(),
      );
      const stable = now === last;
      last = now;
      return stable;
    },
    { interval: 150, timeoutMsg: `${selector} não parou no lugar` },
  );
  await element.click();
}

/** Todos os arquivos de uma pasta, recursivamente. */
function walk(dir: string): string[] {
  try {
    return readdirSync(dir).flatMap((name) => {
      const path = join(dir, name);
      return statSync(path).isDirectory() ? walk(path) : [path];
    });
  } catch {
    return [];
  }
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

describe('Navegar por modpacks (P1-17)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(KEY_FILE, { force: true });
    await resetRequests();
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(KEY_FILE, { force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
  });

  it('CA-T08-14: Tipo = Modpacks e o modpack do Modrinth aberto com até 4 requisições', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await openOrImport();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
    await $('li.toast*=está em Meus packs').waitForDisplayed();
    await dismissToasts();

    await steadyClick('a=Adicionar');
    await $('h1=Adicionar ao pack').waitForDisplayed();
    await $('button[role="radio"]=Modpacks').click();
    const open = 'button[aria-label="Ver os mods de Pacote Leve"]';
    await $(open).waitForDisplayed({ timeout: 30_000 });
    // Sem caixa de seleção nos resultados: um modpack nunca entra no pack.
    expect(await $('ul[aria-label="Resultados"] input[type="checkbox"]').isExisting()).toBe(false);

    const modrinthBefore = (await mockRequests()).filter((line) =>
      line.includes('/modrinth/v2/'),
    ).length;
    await steadyClick(open);
    await $(PANEL_LEVE).$('h2=Mods deste modpack (6)').waitForDisplayed({ timeout: 30_000 });
    const modrinth =
      (await mockRequests()).filter((line) => line.includes('/modrinth/v2/')).length -
      modrinthBefore;
    expect(modrinth).toBeGreaterThan(0);
    expect(modrinth).toBeLessThanOrEqual(4);

    expect(await fullText(PANEL_LEVE)).toContain('Licença do modpack: MIT.');
    expect(await modRow(PANEL_LEVE, 'Sodium')).toContain('No pack');
    expect(await modRow(PANEL_LEVE, 'Lithium')).toContain('No pack');
    expect(await modRow(PANEL_LEVE, 'Rubidium')).toContain(
      'Sem versão para Minecraft 1.21.1 com Fabric',
    );
    expect(await modRow(PANEL_LEVE, 'ajustes-do-autor-1.0')).toContain('Arquivo fora das lojas');
    expect(await modRow(PANEL_LEVE, 'spark')).not.toContain('No pack');
    // O modpack é do mesmo Minecraft e loader: "a mesma do modpack" é o padrão.
    expect(await $(`${PANEL_LEVE} select[aria-label="Versão dos mods"]`).getValue()).toBe('same');
  });

  it('CA-T08-14: "Adicionar 2 selecionados" abre um diálogo só e grava a versão do modpack', async () => {
    await steadyClick(`${PANEL_LEVE} input[aria-label="Selecionar spark"]`);
    await steadyClick(`${PANEL_LEVE} input[aria-label="Selecionar Crash Assistant"]`);
    await steadyClick(
      '//aside[@aria-label="Mods do modpack Pacote Leve"]//button[normalize-space(.)="Adicionar 2 selecionados"]',
    );
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('entrar no pack'),
      { timeout: 30_000, timeoutMsg: 'o plano não chegou' },
    );
    expect(await $$('[role="dialog"]').length).toBe(1);
    const confirm = '//div[@role="dialog"]//button[starts-with(normalize-space(.), "Adicionar ")]';
    await steadyClick(confirm);
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 60_000 });

    // Gravado com a versão que o modpack usa (spark `cALUj9l1`).
    await browser.waitUntil(
      () => {
        try {
          return Promise.resolve(read('mods/spark.pw.toml').includes('cALUj9l1'));
        } catch {
          return Promise.resolve(false);
        }
      },
      { timeout: 30_000, timeoutMsg: 'o spark não foi gravado com a versão do modpack' },
    );
    // A lista do modpack foi relida: os dois agora estão no pack.
    await browser.waitUntil(
      async () =>
        (await modRow(PANEL_LEVE, 'spark')).includes('No pack') &&
        (await modRow(PANEL_LEVE, 'Crash Assistant')).includes('No pack'),
      { timeout: 30_000, timeoutMsg: 'a lista do modpack não mostrou os itens adicionados' },
    );
    expect(await $(`${PANEL_LEVE} [role="region"]`).isExisting()).toBe(false);
  });

  it('CA-T08-15/16: o modpack da CurseForge abre pelo manifest.json lido por partes, só em memória', async () => {
    mkdirSync(join(DATA_ROOT, 'cofre-de-teste'), { recursive: true });
    writeFileSync(KEY_FILE, CHAVE_BOA);
    await steadyClick(
      '//aside[@aria-label="Mods do modpack Pacote Leve"]//button[normalize-space(.)="Fechar o modpack"]',
    );
    await resetRequests();
    const field = $('input[type="search"]');
    await field.click();
    await field.addValue('vale');
    const open = 'button[aria-label="Ver os mods de Vale CurseForge"]';
    await $(open).waitForDisplayed({ timeout: 30_000 });
    await steadyClick(open);
    await $(PANEL_VALE).$('h2=Mods deste modpack (3)').waitForDisplayed({ timeout: 30_000 });

    expect(await modRow(PANEL_VALE, 'Sodium')).toContain('No pack');
    expect(await modRow(PANEL_VALE, 'Mod Só da CurseForge')).not.toContain('No pack');
    expect(await modRow(PANEL_VALE, 'feito-a-mao-1.0')).toContain('Arquivo fora das lojas');
    expect(await fullText(PANEL_VALE)).toContain('A fonte não informa a licença deste modpack.');

    const lines = await mockRequests();
    // O "edge" só recebeu o pedido que resolve o redirecionamento; as partes foram pedidas ao
    // servidor final, e não o arquivo inteiro (o aviso de download inteiro não aparece).
    expect(lines.filter((line) => line.startsWith('GET /cf-edge/'))).toEqual([]);
    const parts = lines.filter((line) => line.startsWith('GET /cf-media/')).length;
    expect(parts).toBeGreaterThan(0);
    expect(parts).toBeLessThanOrEqual(6);
    expect(await fullText(PANEL_VALE)).not.toContain('baixado inteiro');

    // CA-T08-16: nada do manifesto nem da resposta da API ficou na pasta de dados.
    const leaked = walk(DATA_ROOT).filter((path) => {
      const bytes = readFileSync(path);
      return bytes.includes(MANIFEST_MARKER) || bytes.includes(API_MARKER);
    });
    expect(leaked).toEqual([]);
    // O pack também não mudou ao abrir o modpack.
    expect(read('index.toml')).not.toContain('feito-a-mao');
  });
});
