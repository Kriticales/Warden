/**
 * Descoberta no app real (P1-16; SPEC T08, CA-T08-10 e CA-T08-13), com o packwiz de verdade, as
 * respostas gravadas do Modrinth e a CurseForge simulada (servidor de fixtures; nada vai à
 * internet).
 *
 * - Campo vazio: "Populares", "Atualizados recentemente" (na Lista e nos Cartões) e as
 *   categorias, com no máximo 3 requisições ao Modrinth e nenhuma à CurseForge (sem chave).
 * - A categoria "Tecnologia" filtra a busca pela categoria mapeada do Modrinth.
 * - Pré-visualização completa: índice, galeria (no Modrinth, as miniaturas e o visualizador; na
 *   CurseForge, com a chave, a imagem grande servida pelo `warden-img://` a partir do servidor de
 *   fixtures, sem depender da rede), versões com as notas pedidas só ao abrir a linha e
 *   dependências.
 * - Abaixo de 1180 px o Tipo vira uma lista suspensa e a coluna Resumo sai; os filtros ficam.
 * - Depois da sessão com imagens da CurseForge, a pasta de dados do WebView não guarda nenhuma
 *   resposta da CurseForge nem endereço `warden-img://` (o cache de disco do WebView não pode ter
 *   dados da API).
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
import { type ChainablePromiseElement, Key } from 'webdriverio';

import { CHAVE_BOA } from './mock-server/curseforge';
import { dismissToasts } from './toasts';
import { openOrImport } from './open-or-import';

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
/** Onde o cofre de teste (arquivo) guarda a chave da CurseForge (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'curseforge-api-key');

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

/** Zera o registro do servidor de fixtures (os specs anteriores também falam com ele). */
async function resetRequests(): Promise<void> {
  await fetch(`${MOCK_URL}/__reset`, { method: 'POST' });
}

/** Os textos dos botões de nome de uma lista de resultados. */
async function names(listLabel: string): Promise<string[]> {
  return browser.execute(
    (label) =>
      Array.from(
        document.querySelectorAll(`[role="rowgroup"][aria-label="${label}"] .tname__n`),
        (button) => button.textContent,
      ),
    listLabel,
  );
}

async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/**
 * Espera todas as imagens da pré-visualização terminarem (carregadas ou com erro).
 *
 * A descrição e a galeria do Modrinth trazem imagens do CDN dele (internet), com
 * `loading="lazy"` e sem tamanho reservado. O clique do WebDriver vai para um ponto da tela: se
 * uma imagem acima do alvo chega entre a rolagem e o clique, o conteúdo desce e o clique cai em
 * outro lugar (visto na descrição do Sodium, que empurrou a galeria 200 px). Aqui as imagens são
 * pedidas todas de uma vez e o teste só clica com o layout parado.
 */
async function settlePreviewImages(): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const images = Array.from(
          document.querySelectorAll<HTMLImageElement>('aside.disc__preview img'),
        );
        for (const image of images) image.loading = 'eager';
        return images.every((image) => image.complete);
      }),
    { timeout: 30_000, timeoutMsg: 'as imagens da pré-visualização não terminaram de carregar' },
  );
}

/**
 * Clica num elemento da pré-visualização com as imagens paradas, o elemento no meio da coluna
 * e a posição dele igual em duas leituras seguidas (uma seção que termina de carregar acima dele
 * também empurra o conteúdo).
 */
async function clickInPreview(element: ChainablePromiseElement): Promise<void> {
  await settlePreviewImages();
  const target = await element.getElement();
  let last = '';
  await browser.waitUntil(
    async () => {
      const now = await browser.execute((node) => {
        node.scrollIntoView({ block: 'center' });
        const rect = node.getBoundingClientRect();
        return `${String(rect.x)},${String(rect.y)}`;
      }, target);
      const stable = now === last;
      last = now;
      return stable;
    },
    { interval: 150, timeoutMsg: 'o elemento da pré-visualização não parou no lugar' },
  );
  await element.click();
}

/**
 * Abre a linha de uma versão na pré-visualização e espera o `<details>` aberto.
 *
 * Antes, fecha os avisos: eles ficam fixos no canto de baixo à direita, por cima da coluna da
 * pré-visualização, e o clique que cai num aviso é interceptado. A linha vai para o meio da
 * coluna, longe do canto dos avisos e da barra de status. Só depois de a linha estar aberta o
 * teste confere o pedido das notas: um clique que não abriu a linha falha com esse motivo.
 */
async function openVersion(summary: ChainablePromiseElement): Promise<void> {
  await dismissToasts();
  await $('li.toast').waitForExist({ reverse: true });
  await clickInPreview(summary);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (element) => element.closest('details')?.open === true,
        await summary.getElement(),
      ),
    { timeoutMsg: 'a linha da versão não abriu' },
  );
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

  it('CA-T08-10: o início mostra populares, atualizados e categorias com até 3 requisições por fonte', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await openOrImport();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
    // O aviso de que o pack entrou em Meus packs chega logo depois da lista e fica 6 s no canto
    // de baixo à direita, onde a pré-visualização mostra as versões: fechado aqui.
    await $('li.toast*=está em Meus packs').waitForDisplayed();
    await dismissToasts();
    await $('li.toast').waitForExist({ reverse: true });
    searchesBefore = (await mockRequests()).filter((line) =>
      line.includes('/modrinth/v2/search'),
    ).length;

    await $('a=Adicionar').click();
    await $('h1=Adicionar ao pack').waitForDisplayed();
    await $('h2=Populares').waitForDisplayed({ timeout: 30_000 });
    await $('h2=Atualizados recentemente').waitForDisplayed();
    expect((await names('Populares')).length).toBeGreaterThan(0);
    expect(await $('button=Tecnologia').isExisting()).toBe(true);
    expect(await fullText('.lockline')).toContain('Só versões para Minecraft 1.21.1 com Fabric');

    const lines = await mockRequests();
    const searches =
      lines.filter((line) => line.includes('/modrinth/v2/search')).length - searchesBefore;
    expect(searches).toBeGreaterThan(0);
    expect(searches).toBeLessThanOrEqual(3);
    // Sem chave da CurseForge nenhuma requisição vai para ela.
    expect(lines.filter((line) => line.includes('/curseforge/'))).toEqual([]);
    await screenshot('01-inicio');
  });

  it('Lista | Cartões: a seleção continua e o app lembra o modo', async () => {
    // O primeiro dos populares que ainda dá para marcar (o que está no pack não tem caixa).
    const box = $(
      'div[role="rowgroup"][aria-label="Populares"] input[type="checkbox"]:not(:disabled)',
    );
    const label = (await box.getAttribute('aria-label')) ?? '';
    await box.click();
    await $('.selbar*=1 marcado').waitForDisplayed();
    await screenshot('01a-lista-marcado');
    await $('button[role="radio"]=Cartões').click();
    await $('ul.cgrid[aria-label="Populares"]').waitForDisplayed({ timeout: 10_000 });
    expect(await $(`button[aria-label="${label}"]`).getAttribute('aria-pressed')).toBe('true');
    await screenshot('01b-cartoes');
    expect(await browser.execute(() => window.localStorage.getItem('warden.adicionar.modo'))).toBe(
      'cartoes',
    );
    await $('button[role="radio"]=Lista').click();
    await $('div[role="rowgroup"][aria-label="Populares"]').waitForDisplayed();
    await $('.selbar').$('button=Limpar').click();
    await $('.selbar').waitForExist({ reverse: true });
  });

  it('a categoria Tecnologia filtra a busca pela categoria mapeada', async () => {
    await $('button=Tecnologia').click();
    await $('div[role="rowgroup"][aria-label="Tecnologia"]').waitForDisplayed({ timeout: 30_000 });
    const lines = await mockRequests();
    const last = lines.filter((line) => line.includes('/modrinth/v2/search')).at(-1);
    expect(decodeURIComponent(last ?? '')).toContain('categories:technology');
    await screenshot('02-categoria');
    await $('button=Todas').click();
    await $('h2=Populares').waitForDisplayed({ timeout: 30_000 });
  });

  it('CA-T08-13: a pré-visualização mostra galeria, versões com notas sob demanda e dependências', async () => {
    await $('button.tname__n=Sodium').click();
    const preview = $('aside.disc__preview');
    await preview.waitForDisplayed({ timeout: 30_000 });
    expect(await fullText('aside.disc__preview nav[aria-label="Partes desta página"]')).toContain(
      'Galeria',
    );
    // Galeria do Modrinth: miniaturas; clicar abre o visualizador sobre a página. (A imagem do
    // Modrinth vem do CDN dele, na internet; a imagem grande carregada é conferida na CurseForge,
    // abaixo, com o servidor de fixtures.)
    const thumb = $('aside.disc__preview .gallery .gallery__item');
    await thumb.waitForDisplayed({ timeout: 30_000 });
    await clickInPreview(thumb);
    // O diálogo monta depois do clique: o `$` encadeado não espera o pai aparecer.
    const viewer = $('[role="dialog"]');
    await viewer.waitForDisplayed({ timeout: 15_000 });
    await viewer.$('span*=Imagem 1 de 6').waitForDisplayed({ timeout: 15_000 });
    await browser.keys('Escape');
    await $('[role="dialog"]').waitForExist({ reverse: true });

    // Versões: nada de notas até abrir a linha. A primeira linha é a versão escolhida, que as
    // Dependências já leram (com as notas, por `/versions?ids=`); a segunda ainda não foi lida.
    const asked = async () =>
      (await mockRequests()).filter((line) => line.includes('/modrinth/v2/version/')).length;
    const askedBefore = await asked();
    const firstVersion = $('aside.disc__preview ul.versions li:nth-child(2) summary');
    await firstVersion.waitForDisplayed({ timeout: 30_000 });
    expect(await asked()).toBe(askedBefore);
    await openVersion(firstVersion);
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

  it('CA-T08-13: na CurseForge, a galeria abre a imagem grande pelo warden-img:// e as notas esperam a linha', async () => {
    mkdirSync(join(DATA_ROOT, 'cofre-de-teste'), { recursive: true });
    writeFileSync(KEY_FILE, CHAVE_BOA);
    const field = $('input[type="search"]');
    await field.click();
    await field.addValue('mod');
    await $('button.tname__n=Mod Só da CurseForge').waitForDisplayed({ timeout: 30_000 });
    await $('button.tname__n=Mod Só da CurseForge').click();
    await $('aside.disc__preview')
      .$('h2=Mod Só da CurseForge')
      .waitForDisplayed({ timeout: 30_000 });

    const thumb = $('aside.disc__preview .gallery .gallery__item');
    await thumb.waitForDisplayed({ timeout: 30_000 });
    // A miniatura vem pelo protocolo, carregada (não é o endereço do servidor de imagens).
    const thumbSrc = await $('aside.disc__preview .gallery .gallery__item img').getAttribute('src');
    expect(thumbSrc).toMatch(/warden-img/);
    expect(thumbSrc).not.toContain('curseforge-img');
    await clickInPreview(thumb);
    const big = $('[role="dialog"] img.gallery__big');
    await big.waitForDisplayed({ timeout: 15_000 });
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const img = document.querySelector<HTMLImageElement>('[role="dialog"] img.gallery__big');
          return img !== null && img.complete && img.naturalWidth === 320;
        }),
      { timeout: 15_000, timeoutMsg: 'a imagem grande da CurseForge não carregou' },
    );
    await screenshot('03-galeria');
    await browser.keys('Escape');
    await $('[role="dialog"]').waitForExist({ reverse: true });

    const changelogs = async () =>
      (await mockRequests()).filter((line) =>
        /\/curseforge\/mods\/\d+\/files\/\d+\/changelog/.test(line),
      ).length;
    const before = await changelogs();
    const firstVersion = $('aside.disc__preview ul.versions summary');
    await firstVersion.waitForDisplayed({ timeout: 30_000 });
    expect(await changelogs()).toBe(before);
    await openVersion(firstVersion);
    await browser.waitUntil(async () => (await changelogs()) > before, {
      timeout: 30_000,
      timeoutMsg: 'as notas da CurseForge não foram pedidas ao abrir a linha',
    });
    // As notas (HTML da CurseForge) aparecem higienizadas na linha aberta.
    await $('aside.disc__preview ul.versions')
      .$('p=Notas simuladas desta versão.')
      .waitForDisplayed({
        timeout: 30_000,
      });
    await screenshot('04b-previa-curseforge');

    // Volta ao início, sem a pré-visualização da CurseForge aberta.
    await field.click();
    await browser.keys([Key.Ctrl, 'a']);
    await browser.keys(Key.Backspace);
    await $('h2=Populares').waitForDisplayed({ timeout: 30_000 });
    await $('button.tname__n=Sodium').click();
    await $('aside.disc__preview').$('h2=Sodium').waitForDisplayed({ timeout: 30_000 });
  });

  it('abaixo de 1180 px o Tipo vira lista suspensa e o Resumo sai; os filtros continuam', async () => {
    await $('button=Filtros').click();
    await $('aside[aria-label="Filtros"]').waitForDisplayed({ timeout: 10_000 });
    await browser.setWindowSize(1100, 800);
    await $('select[aria-label="Tipo"]').waitForDisplayed({ timeout: 10_000 });
    expect(await $('div[role="radiogroup"][aria-label="Tipo"]').isDisplayed()).toBe(false);
    expect(await $('button=Todas').isDisplayed()).toBe(true);
    await screenshot('05-janela-estreita');
    await browser.setWindowSize(1366, 860);
    await browser.waitUntil(
      async () => $('div[role="radiogroup"][aria-label="Tipo"]').isDisplayed(),
      { timeoutMsg: 'o Tipo deveria voltar a ser o seletor segmentado em tela larga' },
    );
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
