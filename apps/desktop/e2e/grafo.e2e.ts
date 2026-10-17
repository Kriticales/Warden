/**
 * Modo Grafo da seção Mods no app real (D-11; SPEC T06, CA-T06-07): o grafo do Rust montado
 * a partir dos jars da instância de teste, o desenho focado com os nós como botões, a lista em
 * texto, "Ver no grafo" nos detalhes, "Ver o pack inteiro" com o Cytoscape e o axe no WebView
 * (aqui o contraste das cores é medido de verdade).
 *
 * O pack é o do Fabric 1.21.1 gerado pelo packwiz real
 * (`crates/warden-packwiz/tests/fixtures/packwiz-output/fabric-1.21.1`). Os jars da instância
 * são sintéticos (`storedZip`), só com o `fabric.mod.json`: o Sodium exige o Lithium e recomenda
 * o Mod Menu; o Jade exige o Sodium.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';
import type { ChainablePromiseElement } from 'webdriverio';

import { seriousViolationSummaries } from './axe';
import { storedZip } from './mock-server/jars';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
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
const PACK = join(DATA_ROOT, 'fora', 'pack-grafo');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

/** Um jar Fabric só com o `fabric.mod.json`. */
function fabricJar(fields: Record<string, unknown>): Buffer {
  return storedZip({ 'fabric.mod.json': JSON.stringify({ schemaVersion: 1, ...fields }) });
}

/** Instância de teste criada por este spec (apagada no fim). */
let instanceDir: string | null = null;

/** Põe os jars na instância de teste do pack, onde o grafo os procura. */
function writeInstanceJars(): void {
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs: { id: string; path: string }[];
  };
  const id = registry.packs.find((pack) => pack.path.endsWith('pack-grafo'))?.id;
  if (!id) throw new Error('pack-grafo não está no registro');
  instanceDir = join(DATA_ROOT, 'data', 'instances', id);
  const mods = join(instanceDir, 'minecraft', 'mods');
  mkdirSync(mods, { recursive: true });
  const jars: Record<string, Buffer> = {
    'sodium-fabric-0.8.13+mc1.21.1.jar': fabricJar({
      id: 'sodium',
      version: '0.8.13',
      depends: { minecraft: '1.21.1', lithium: '*' },
      recommends: { modmenu: '*' },
    }),
    'lithium-fabric-0.15.4+mc1.21.1.jar': fabricJar({ id: 'lithium', version: '0.15.4' }),
    'modmenu-11.0.5.jar': fabricJar({ id: 'modmenu', version: '11.0.5' }),
    'Jade-1.21.1-Fabric-15.10.6.jar': fabricJar({
      id: 'jade',
      version: '15.10.6',
      depends: { sodium: '*' },
    }),
  };
  for (const [name, bytes] of Object.entries(jars)) writeFileSync(join(mods, name), bytes);
}

/** O grafo focado com o mod no centro pelo nome. */
function graphOf(name: string) {
  return $(`[role="group"][aria-label="Grafo de dependências de ${name}"]`);
}

/**
 * Ativa um botão ou link pelo teclado (foco e Enter), como quem usa o Tab. Na CI do Linux o
 * clique por coordenada às vezes cai fora do alvo quando a tela ainda se ajusta (linhas e
 * detalhes que terminam de carregar) e o clique se perde sem erro.
 */
async function activate(target: ChainablePromiseElement): Promise<void> {
  await target.waitForClickable();
  const focused = await browser.execute(
    (node) => {
      node.focus();
      return document.activeElement === node;
    },
    await target.getElement(),
  );
  expect(focused).toBe(true);
  await browser.keys('Enter');
}

/** Texto completo de um elemento (inclusive o que está fora da área visível). */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

describe('Mods no modo Grafo (D-11)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados vazia para os specs seguintes (o test.e2e.ts espera uma
    // instância só, a dele).
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    if (instanceDir) rmSync(instanceDir, { recursive: true, force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('CA-T06-07: Sodium no centro, o que ele exige à esquerda e quem precisa dele à direita', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
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
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
    writeInstanceJars();

    await dismissToasts();
    await activate($('button[role="radio"]=Grafo'));
    const label = $('label=Mod no centro');
    await label.waitForDisplayed();
    const selectId = String(await label.getAttribute('for'));
    await $(`[id="${selectId}"]`).selectByVisibleText('Sodium');
    const graph = graphOf('Sodium');
    await graph.waitForDisplayed({ timeout: 30_000 });

    const columns = await browser.execute(() =>
      Array.from(document.querySelectorAll('.graph ul.gcol')).map((list) => list.textContent),
    );
    expect(columns[0]).toContain('Lithium');
    expect(columns[0]).toContain('Mod Menu');
    expect(columns[0]).toContain('recomendada');
    expect(columns[1]).toContain('Jade');
    const links = await fullText('section[aria-labelledby="grafo-ligacoes"]');
    expect(links).toContain('Sodium precisa de Lithium · obrigatória');
    expect(links).toContain('Sodium recomenda Mod Menu · recomendada');
    expect(links).toContain('Jade 🔍 precisa de Sodium · obrigatória');
    // "Se você remover" vem de outra consulta (a dos dependentes): espera ela chegar.
    let words = '';
    await browser.waitUntil(
      async () => {
        words = await fullText('section[aria-labelledby="grafo-palavras"]');
        return words.includes('Se você remover');
      },
      { timeout: 30_000, timeoutMsg: `"Se você remover" não apareceu (${words})` },
    );
    expect(words).toContain('Se você remover o Sodium, 1 mod para de funcionar: Jade 🔍');
    await screenshot('60-mods-grafo');
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('nós são botões: Enter num nó abre os detalhes; "Ver no grafo" põe o mod no centro', async () => {
    const jade = graphOf('Sodium').$('button*=Jade');
    await jade.waitForClickable();
    // Chega ao nó pelo foco (como o Tab) e abre com Enter, sem clique.
    const focused = await browser.execute(() => {
      const node = Array.from(document.querySelectorAll<HTMLButtonElement>('.graph .gnode')).find(
        (button) => button.textContent.includes('Jade'),
      );
      node?.focus();
      return document.activeElement === node;
    });
    expect(focused).toBe(true);
    await browser.keys('Enter');
    const drawer = $('[role="dialog"]');
    await drawer.waitForDisplayed();
    // Os blocos dos detalhes carregam aos poucos e empurram o link: espera as dependências
    // chegarem (o bloco logo acima do link) antes de clicar.
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('Depende de'),
      {
        timeout: 30_000,
        timeoutMsg: 'as dependências do Jade não carregaram nos detalhes',
      },
    );
    await dismissToasts();
    await activate(drawer.$('a=Ver no grafo'));
    await drawer.waitForExist({ reverse: true });
    await graphOf('Jade 🔍').waitForDisplayed({ timeout: 30_000 });
    await expect($('button[role="radio"][aria-checked="true"]')).toHaveText('Grafo');
  });

  it('"Ver o pack inteiro" desenha com o Cytoscape e mostra a lista em texto', async () => {
    await dismissToasts();
    const open = $('button=Ver o pack inteiro');
    await open.waitForClickable();
    await open.click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    const canvas = dialog.$('.grafo-inteiro__canvas');
    await browser.waitUntil(async () => (await canvas.getAttribute('data-draw-ms')) !== null, {
      timeout: 30_000,
      timeoutMsg: 'o desenho do pack inteiro não terminou',
    });
    expect(await dialog.$('canvas').isExisting()).toBe(true);
    const list = await fullText('.grafo-inteiro__lista');
    expect(list).toContain('Sodium');
    expect(list).toContain('precisa de Lithium');
    await screenshot('61-pack-inteiro');
    expect(await seriousViolationSummaries()).toEqual([]);
    await browser.keys('Escape');
    await dialog.waitForExist({ reverse: true });
    await browser.waitUntil(
      async () =>
        browser.execute(() => document.activeElement?.textContent === 'Ver o pack inteiro'),
      { timeoutMsg: 'o foco não voltou para "Ver o pack inteiro"' },
    );
  });
});
