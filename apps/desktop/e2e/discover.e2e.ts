/**
 * Descoberta no app real (P1-16; SPEC T08, CA-T08-10 e CA-T08-13), com o packwiz de verdade e as
 * respostas gravadas do Modrinth (servidor de fixtures; nada vai à internet).
 *
 * - Campo vazio: "Populares para Minecraft 1.21.1 com Fabric", "Atualizados recentemente" e as
 *   categorias, com no máximo 3 requisições ao Modrinth e nenhuma à CurseForge (sem chave).
 * - A categoria "Tecnologia" filtra a busca pela categoria mapeada do Modrinth.
 * - Pré-visualização completa: índice, galeria com a imagem grande, versões com as notas pedidas
 *   só ao abrir a linha e dependências.
 * - Abaixo de 1180 px a coluna de filtros recolhe atrás do botão "Filtros".
 * - Depois da sessão, a pasta de dados do WebView não guarda nenhuma resposta da CurseForge nem
 *   endereço `warden-img://` (o cache de disco do WebView não pode ter dados da API).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
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

import { $, browser, expect } from '@wdio/globals';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-descoberta');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const WEBVIEW_DIR = join(DATA_ROOT, 'data', 'EBWebView');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

/** Tira o Sodium do pack copiado, para ele aparecer nas listas como algo a adicionar. */
function removeSodium(): void {
  rmSync(join(PACK, 'mods', 'sodium.pw.toml'));
  const index = read('index.toml').replace(
    /\[\[files\]\]\nfile = "mods\/sodium\.pw\.toml"\nhash = "[0-9a-f]+"\nmetafile = true\n\n/,
    '',
  );
  writeFileSync(join(PACK, 'index.toml'), index);
  const hash = createHash('sha256').update(index).digest('hex');
  writeFileSync(
    join(PACK, 'pack.toml'),
    read('pack.toml').replace(/^hash = "[0-9a-f]+"$/m, `hash = "${hash}"`),
  );
}

/** As requisições que o servidor de fixtures recebeu até agora. */
async function mockRequests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

/** Os textos dos botões de nome de uma lista de resultados. */
async function names(listLabel: string): Promise<string[]> {
  return browser.execute(
    (label) =>
      Array.from(
        document.querySelectorAll(`ul[aria-label="${label}"] .drow .drow__name`),
        (button) => button.textContent,
      ),
    listLabel,
  );
}

async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** Todos os arquivos de uma pasta, recursivamente (a pasta pode nem existir). */
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

describe('Descoberta (P1-16)', () => {
  let searchesBefore = 0;

  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    removeSodium();
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
  });

  it('CA-T08-10: o início mostra populares, atualizados e categorias com até 3 requisições por fonte', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
    searchesBefore = (await mockRequests()).filter((line) =>
      line.includes('/modrinth/v2/search'),
    ).length;

    await $('a=Adicionar').click();
    await $('h1=Adicionar ao pack').waitForDisplayed();
    await $('h2=Populares para Minecraft 1.21.1 com Fabric').waitForDisplayed({ timeout: 30_000 });
    await $('h2=Atualizados recentemente').waitForDisplayed();
    expect((await names('Populares para Minecraft 1.21.1 com Fabric')).length).toBeGreaterThan(0);
    expect(await $('label=Tecnologia').isExisting()).toBe(true);

    const lines = await mockRequests();
    const searches =
      lines.filter((line) => line.includes('/modrinth/v2/search')).length - searchesBefore;
    expect(searches).toBeGreaterThan(0);
    expect(searches).toBeLessThanOrEqual(3);
    // Sem chave da CurseForge nenhuma requisição vai para ela.
    expect(lines.filter((line) => line.includes('/curseforge/'))).toEqual([]);
    await screenshot('01-inicio');
  });

  it('a categoria Tecnologia filtra a busca pela categoria mapeada', async () => {
    await $('label*=Tecnologia').click();
    await $('ul[aria-label="Resultados"]').waitForDisplayed({ timeout: 30_000 });
    const lines = await mockRequests();
    const last = lines.filter((line) => line.includes('/modrinth/v2/search')).at(-1);
    expect(decodeURIComponent(last ?? '')).toContain('categories:technology');
    await screenshot('02-categoria');
    await $('label=Todas as categorias').click();
    await $('h2=Populares para Minecraft 1.21.1 com Fabric').waitForDisplayed({ timeout: 30_000 });
  });

  it('CA-T08-13: a pré-visualização mostra galeria, versões com notas sob demanda e dependências', async () => {
    await $('button.drow__name=Sodium').click();
    const preview = $('aside.disc__preview');
    await preview.waitForDisplayed({ timeout: 30_000 });
    expect(await fullText('aside.disc__preview nav[aria-label="Partes desta página"]')).toContain(
      'Galeria',
    );
    // Galeria: miniaturas; clicar abre a imagem grande sobre a página.
    const thumb = $('aside.disc__preview .gallery .gallery__item');
    await thumb.waitForDisplayed({ timeout: 30_000 });
    await thumb.click();
    await $('[role="dialog"] img.gallery__big').waitForDisplayed({ timeout: 15_000 });
    await screenshot('03-galeria');
    await browser.keys('Escape');
    await $('[role="dialog"]').waitForExist({ reverse: true });

    // Versões: nada de notas até abrir a linha.
    const asked = async () =>
      (await mockRequests()).filter((line) => line.includes('/modrinth/v2/version/')).length;
    const askedBefore = await asked();
    const firstVersion = $('aside.disc__preview ul.versions summary');
    await firstVersion.waitForDisplayed({ timeout: 30_000 });
    expect(await asked()).toBe(askedBefore);
    await firstVersion.click();
    await browser.waitUntil(async () => (await asked()) > askedBefore, {
      timeout: 30_000,
      timeoutMsg: 'as notas não foram pedidas ao abrir a linha',
    });
    // Dependências do Sodium: a lista com as três linhas (obrigatória, opcional, incompatível).
    const deps = await fullText('aside.disc__preview');
    expect(deps).toContain('Dependências');
    expect(deps).toContain('Obrigatória');
    expect(deps).toContain('Incompatível');
    await screenshot('04-previa-completa');
  });

  it('abaixo de 1180 px a coluna de filtros recolhe atrás do botão "Filtros"', async () => {
    await browser.setWindowSize(1100, 800);
    const button = $('button=Filtros');
    await button.waitForDisplayed({ timeout: 10_000 });
    expect(await $('label=Todas as categorias').isDisplayed()).toBe(false);
    await button.click();
    await $('label=Todas as categorias').waitForDisplayed();
    await screenshot('05-filtros-recolhidos');
    await browser.setWindowSize(1366, 860);
    await browser.waitUntil(async () => !(await $('button=Filtros').isDisplayed()), {
      timeoutMsg: 'o botão Filtros deveria sumir em tela larga',
    });
  });

  it('a pasta do WebView não guarda dados da CurseForge nem endereços warden-img', async () => {
    const hits = walk(WEBVIEW_DIR).filter((file) => {
      try {
        const text = readFileSync(file, 'latin1');
        return /warden-img|api\.curseforge\.com|forgecdn\.net/i.test(text);
      } catch {
        return false;
      }
    });
    expect(hits).toEqual([]);
    const images = await browser.execute(
      () =>
        document.querySelectorAll('img[src^="warden-img"], img[src*="warden-img.localhost"]')
          .length,
    );
    expect(images).toBe(0);
  });
});
