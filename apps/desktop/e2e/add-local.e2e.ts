/**
 * Link e arquivo do computador em Adicionar no app real (P1-11; SPEC T08), com o packwiz de
 * verdade e o Modrinth simulado (servidor de fixtures; nada vai à internet).
 *
 * Os packs são os que o packwiz real gerou (`crates/warden-packwiz/tests/fixtures/
 * packwiz-output/`): Fabric 1.21.1 sem o Sodium e Forge 1.20.1. Os jars são sintéticos
 * (`mock-server/jars.ts`) e ficam numa pasta com espaços e acentos, como no Windows.
 *
 * - `http://` é recusado sem nenhum download: o servidor (que existe e responderia) não recebe
 *   nada (CA-T08-07).
 * - Link de projeto do Modrinth abre a pré-visualização.
 * - Link direto `https://` é baixado uma vez e gravado como referência com o link e o `sha256`.
 * - O jar do Sodium, achado pelo hash no Modrinth, vira referência: `mods/sodium.pw.toml` com
 *   `[update.modrinth]`, sem copiar o jar (CA-T08-04).
 * - Um jar Fabric num pack Forge mostra o bloqueio com a explicação; "Adicionar mesmo assim"
 *   copia o arquivo para `mods/` (CA-T08-05; o erro de loader no diagnóstico é conferido no
 *   teste de integração `crates/warden-project/tests/add_local.rs`).
 *
 * O diálogo nativo de arquivos é trocado pelo arquivo de `WARDEN_E2E_PICK_FILES`; o link direto
 * vai ao servidor de fixtures por `WARDEN_E2E_DIRECT_VIA` (só em build de debug).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';
import { Key } from 'webdriverio';

import { seriousViolationSummaries } from './axe';
import { DIRECT_LINK, directJar, fabricJar, sodiumJar } from './mock-server/jars';
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
const FIXTURES = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  'crates',
  'warden-packwiz',
  'tests',
  'fixtures',
  'packwiz-output',
);
const FABRIC_PACK = join(DATA_ROOT, 'fora', 'pack-arquivo-fabric');
const FORGE_PACK = join(DATA_ROOT, 'fora', 'pack-arquivo-forge');
const DOWNLOADS = join(DATA_ROOT, 'Área de Trabalho', 'Mods baixados');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `local-${name}.png`));
}

function read(pack: string, path: string): string {
  return readFileSync(join(pack, path), 'utf8').replace(/\r\n/g, '\n');
}

/** Tira o Sodium do pack copiado, como se ele nunca tivesse entrado (índice e hash dele). */
function removeSodium(pack: string): void {
  rmSync(join(pack, 'mods', 'sodium.pw.toml'));
  const index = read(pack, 'index.toml').replace(
    /\[\[files\]\]\nfile = "mods\/sodium\.pw\.toml"\nhash = "[0-9a-f]+"\nmetafile = true\n\n/,
    '',
  );
  expect(index).not.toContain('sodium');
  writeFileSync(join(pack, 'index.toml'), index);
  const hash = createHash('sha256').update(index).digest('hex');
  writeFileSync(
    join(pack, 'pack.toml'),
    read(pack, 'pack.toml').replace(/^hash = "[0-9a-f]+"$/m, `hash = "${hash}"`),
  );
}

/** Os arquivos que o "diálogo" vai devolver na próxima escolha. */
function pickFiles(...paths: string[]): void {
  writeFileSync(PICK_FILES, paths.join('\n'));
}

async function requests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

async function resetRequests(): Promise<void> {
  await fetch(`${MOCK_URL}/__reset`);
}

/** Texto completo de um elemento (inclusive o que está fora da tela). */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
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

/** Troca o texto do campo único (o WebDriver digita letra a letra). */
/**
 * Troca o texto do campo único. O `clearValue` do WebDriver não dispara o evento `input` que o
 * React ouve (o campo voltaria com o texto antigo): o texto sai como o usuário o apaga.
 */
async function typeInField(text: string): Promise<void> {
  const field = $('input[type="search"]');
  await field.waitForDisplayed();
  await field.click();
  await browser.keys([Key.Ctrl, 'a']);
  await browser.keys(Key.Backspace);
  await browser.waitUntil(async () => (await field.getValue()) === '', {
    timeoutMsg: 'o campo único não ficou vazio',
  });
  if (text !== '') await field.addValue(text);
}

async function openPack(folder: string, title: string): Promise<void> {
  writeFileSync(PICK_FOLDER, folder);
  await $('button=Abrir ou importar…').click();
  await $('h1=Abrir pack').waitForDisplayed();
  await $('button=Abrir pack').click();
  await $(`h1=${title}`).waitForDisplayed({ timeout: 60_000 });
  await $('h1=Mods').waitForDisplayed();
  await $('a=Adicionar').waitForDisplayed({ timeout: 30_000 });
  await $('a=Adicionar').click();
  await $('h1=Adicionar ao pack').waitForDisplayed();
}

async function chooseFiles(...paths: string[]): Promise<void> {
  pickFiles(...paths);
  await $('button=Escolher arquivo do computador…').click();
  await $('ul[aria-label="Arquivos para adicionar"]').waitForDisplayed({ timeout: 30_000 });
}

describe('Adicionar por link e arquivo do computador (P1-11)', () => {
  before(async () => {
    for (const pack of [FABRIC_PACK, FORGE_PACK]) rmSync(pack, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(join(FIXTURES, 'fabric-1.21.1'), FABRIC_PACK, { recursive: true });
    removeSodium(FABRIC_PACK);
    cpSync(join(FIXTURES, 'forge-1.20.1'), FORGE_PACK, { recursive: true });
    mkdirSync(DOWNLOADS, { recursive: true });
    writeFileSync(join(DOWNLOADS, 'sodium-fabric-0.8.13+mc1.21.1.jar'), sodiumJar);
    writeFileSync(join(DOWNLOADS, 'Mod Fabric (teste) ç.jar'), fabricJar);
    writeFileSync(PICK_FOLDER, '');
    pickFiles();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados vazia para os testes que rodam depois.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
    rmSync(join(DATA_ROOT, 'Área de Trabalho'), { recursive: true, force: true });
    writeFileSync(PICK_FILES, '');
  });

  it('http:// é recusado com mensagem e nada é baixado (CA-T08-07)', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    await openPack(FABRIC_PACK, 'Fixture Fabric');
    await resetRequests();
    // Um endereço que existe e responderia: o servidor de fixtures, por http.
    await typeInField(`${MOCK_URL}/direto/files.example.com/mods/Mod%20do%20Link%202.0.jar`);
    await $('.alert*=Links http:// não são aceitos.').waitForDisplayed({ timeout: 15_000 });
    expect(await fullText('.disc__main')).toContain('Nada foi baixado.');
    await browser.pause(800);
    expect((await requests()).filter((line) => line.includes('/direto/'))).toEqual([]);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('01-http-recusado');
  });

  it('link de projeto do Modrinth abre a pré-visualização', async () => {
    await typeInField('https://modrinth.com/mod/sodium');
    const preview = $('aside.disc__preview');
    await browser.waitUntil(
      async () => (await fullText('aside.disc__preview')).includes('Sodium'),
      {
        timeout: 30_000,
        timeoutMsg: 'a pré-visualização do Sodium não abriu',
      },
    );
    await preview.waitForDisplayed();
    await browser.waitUntil(
      async () => (await fullText('aside.disc__preview')).includes('(mais nova compatível)'),
      { timeout: 30_000, timeoutMsg: 'seletor de versão sem a versão padrão' },
    );
    await screenshot('02-link-modrinth');
  });

  it('link direto https é baixado uma vez e gravado com o link e o sha256', async () => {
    await resetRequests();
    await typeInField(DIRECT_LINK);
    const list = $('ul[aria-label="Arquivos para adicionar"]');
    await list.waitForDisplayed({ timeout: 30_000 });
    expect(await fullText('ul[aria-label="Arquivos para adicionar"]')).toContain(
      'Baixado de files.example.com',
    );
    expect((await requests()).filter((line) => line.includes('/direto/'))).toHaveLength(1);
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('03-link-direto');
    await list.$('button=Adicionar pelo link').click();
    await waitToast('1 arquivo adicionado ao pack');
    const metafile = read(FABRIC_PACK, 'mods/mod-do-link.pw.toml');
    expect(metafile).toContain(`url = "${DIRECT_LINK}"`);
    expect(metafile).toContain('hash-format = "sha256"');
    expect(metafile).toContain(`hash = "${createHash('sha256').update(directJar).digest('hex')}"`);
    expect(read(FABRIC_PACK, 'index.toml')).toContain('mods/mod-do-link.pw.toml');
    expect(existsSync(join(FABRIC_PACK, 'mods', 'Mod do Link 2.0.jar'))).toBe(false);
    await dismissToasts();
  });

  it('o jar do Sodium é achado no Modrinth e vira referência, sem copiar o jar (CA-T08-04)', async () => {
    await typeInField('');
    await chooseFiles(join(DOWNLOADS, 'sodium-fabric-0.8.13+mc1.21.1.jar'));
    const list = $('ul[aria-label="Arquivos para adicionar"]');
    await list.$('.alert*=Este arquivo é o Sodium').waitForDisplayed({ timeout: 30_000 });
    expect(await fullText('ul[aria-label="Arquivos para adicionar"]')).toContain(
      'Este arquivo é o Sodium mc1.21.1-0.8.13-fabric do Modrinth.',
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('04-identificado');
    await list.$('button=Adicionar como referência ao Modrinth (recomendado)').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('O que você escolheu'),
      { timeout: 30_000, timeoutMsg: 'o plano não apareceu no diálogo' },
    );
    await dialog.$('button=Adicionar 1 item').click();
    await waitToast('1 item adicionado ao pack');
    await dialog.waitForExist({ reverse: true });
    const sodium = read(FABRIC_PACK, 'mods/sodium.pw.toml');
    expect(sodium).toContain('[update.modrinth]');
    expect(sodium).toContain('mod-id = "AANobbMI"');
    expect(sodium).toContain('hash-format = "sha512"');
    const jars = readdirSync(join(FABRIC_PACK, 'mods')).filter((name) => name.endsWith('.jar'));
    expect(jars).toEqual([]);
    await dismissToasts();
  });

  it('um jar Fabric num pack Forge mostra o bloqueio; "Adicionar mesmo assim" copia (CA-T08-05)', async () => {
    await $('header.packhead').$('a*=Meus packs').click();
    await $('h1=Meus packs').waitForDisplayed();
    await openPack(FORGE_PACK, 'Fixture Forge');
    expect(await $('body').getText()).toContain('Minecraft 1.20.1 com Forge');
    await chooseFiles(join(DOWNLOADS, 'Mod Fabric (teste) ç.jar'));
    const list = $('ul[aria-label="Arquivos para adicionar"]');
    const block = list.$('.alert*=Este arquivo não serve para este pack.');
    await block.waitForDisplayed({ timeout: 30_000 });
    const text = await fullText('ul[aria-label="Arquivos para adicionar"]');
    expect(text).toContain('Ele é um mod para Fabric, e este pack usa Forge.');
    expect(text).toContain('aparece como problema no diagnóstico');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('05-bloqueio');
    await block.$('button=Adicionar mesmo assim').click();
    await waitToast('1 arquivo adicionado ao pack');
    expect(readFileSync(join(FORGE_PACK, 'mods', 'Mod Fabric (teste) ç.jar'))).toEqual(fabricJar);
    expect(read(FORGE_PACK, 'index.toml')).toContain('mods/Mod Fabric (teste) ç.jar');
    await dismissToasts();
    await $('a*=Voltar para Mods').click();
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Mod Fabric (teste) ç').waitForDisplayed({ timeout: 30_000 });
    await screenshot('06-na-lista');
  });
});
