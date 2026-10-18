/**
 * Saúde do pack no app real (D-08; SPEC T14 e T02), com o packwiz de verdade e o Modrinth
 * simulado (servidor de fixtures).
 *
 * - Problemas mostra a nota no topo: o Sodium está como "Cliente e servidor", mas o Modrinth diz
 *   que só funciona no cliente (aviso, −3), e o pack nunca foi testado (−10): 87, Bom, com "O que
 *   tirou pontos" na ordem da tabela e o link para o item.
 * - "Ignorar neste pack" devolve os 3 pontos na hora (90, Ótimo) e a linha do aviso fica riscada.
 * - Meus packs mostra a mesma nota e faixa na coluna Saúde (gravada em `packs.json`); um pack que
 *   nunca passou pelo diagnóstico mostra "Sem dados", e abrir a lista não roda o diagnóstico
 *   (CA-T02-05).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
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
const PACK = join(DATA_ROOT, 'fora', 'pack-saude');
const NEW_PACK = join(DATA_ROOT, 'fora', 'pack-saude-novo');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const REPORTS_DIR = join(DATA_ROOT, 'data', 'diagnostics');

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** O Sodium como o packwiz grava (versão gravada `SMxNOGZ6`), nos dois lados. */
const SODIUM = [
  'name = "Sodium"',
  'filename = "sodium-fabric-0.8.13+mc1.21.1.jar"',
  'side = "both"',
  '',
  '[download]',
  'url = "https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar"',
  'hash-format = "sha512"',
  'hash = "8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e54e8be06d5ea5a9c268beb0e02073209d1f831a02dede851588b972367680853"',
  '',
  '[update]',
  '[update.modrinth]',
  'mod-id = "AANobbMI"',
  'version = "SMxNOGZ6"',
  '',
].join('\n');

/** Um pack Fabric 1.21.1 com os arquivos dados no índice. */
function writePack(root: string, name: string, files: Record<string, string>): void {
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });
  const entries = Object.entries(files).map(([path, content]) => {
    const file = join(root, path);
    mkdirSync(join(file, '..'), { recursive: true });
    writeFileSync(file, content);
    return `[[files]]\nfile = "${path}"\nhash = "${sha256(content)}"\nmetafile = true\n`;
  });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(root, 'index.toml'), index);
  writeFileSync(
    join(root, 'pack.toml'),
    [
      `name = "${name}"`,
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
      'fabric = "0.16.10"',
      'minecraft = "1.21.1"',
      '',
    ].join('\n'),
  );
}

interface Registry {
  packs?: { id: string; path: string; health?: { score: number; band: string } }[];
}

function registry(): Registry {
  return JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as Registry;
}

function packId(root: string): string {
  const id = registry().packs?.find((pack) => pack.path === root)?.id;
  if (!id) throw new Error(`pack não registrado: ${root}`);
  return id;
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

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

/** O bloco da saúde no topo de Problemas. */
function healthBlock() {
  return $('section.health-panel');
}

/** A nota grande, pelo rótulo ("Saúde do pack: 87 de 100, Bom"). */
function healthMeter(label: string) {
  return healthBlock().$(`[role="group"][aria-label="${label}"]`);
}

/** Abre uma pasta pelo "Abrir ou importar…" de Meus packs. */
async function openFolder(root: string): Promise<void> {
  writeFileSync(PICK_FOLDER, root);
  await dismissToasts();
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
}

/** De um pack aberto para Meus packs. */
async function backToPacks(): Promise<void> {
  await dismissToasts();
  await $('header.packhead').$('a*=Meus packs').click();
  await $('h1=Meus packs').waitForDisplayed({ timeout: 30_000 });
}

/** A célula Saúde da linha de um pack. */
function healthCell(name: string) {
  return $(`tr.packrow*=${name}`).$('.health--sm');
}

describe('Saúde do pack: Problemas e Meus packs (D-08; T14 e T02)', function () {
  before(async function () {
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack(PACK, 'Vale Saudável', { 'mods/sodium.pw.toml': SODIUM });
    writePack(NEW_PACK, 'Pack Sem Nota', {});
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    if (existsSync(REGISTRY_FILE)) {
      for (const pack of registry().packs ?? []) {
        rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
        rmSync(join(REPORTS_DIR, `${pack.id}.json`), { force: true });
      }
    }
    rmSync(PACK, { recursive: true, force: true });
    rmSync(NEW_PACK, { recursive: true, force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('Problemas mostra a nota com "O que tirou pontos" e o link para o item', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    await openFolder(PACK);
    await sectionLink('Problemas').click();
    await waitHeading('Problemas');
    const meter = healthMeter('Saúde do pack: 87 de 100, Bom');
    await meter.waitForDisplayed({ timeout: 60_000 });
    await expect(meter).toHaveText(expect.stringContaining('Bom'));
    await expect(healthBlock()).toHaveText(
      expect.stringContaining('Resumo dos problemas conhecidos. Não garante que o pack funciona.'),
    );

    await healthBlock().$('summary=O que tirou pontos').click();
    const rows = healthBlock().$$('ul.losses > li');
    await browser.waitUntil(async () => (await rows.length) === 2, {
      timeout: 10_000,
      timeoutMsg: '"O que tirou pontos" não mostrou as 2 linhas',
    });
    const texts = await rows.map((row) => row.getText());
    expect(texts[0]).toContain('−3');
    expect(texts[0]).toContain('Problemas encontrados: 1 aviso: Sodium está no lado errado.');
    expect(texts[1]).toContain('−10');
    expect(texts[1]).toContain('Último teste: o pack ainda não foi testado.');
    const link = healthBlock().$('a=Sodium está no lado errado');
    expect(await link.getAttribute('href')).toContain('item=mods%2Fsodium.pw.toml');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d08-01-saude-em-problemas');
  });

  it('"Ignorar neste pack" devolve os pontos e a linha do aviso fica riscada', async () => {
    await dismissToasts();
    const card = $('article.issue*=Sodium está no lado errado');
    const ignore = card.$('button=Ignorar neste pack');
    await ignore.waitForClickable({ timeout: 30_000 });
    await ignore.click();
    await healthMeter('Saúde do pack: 90 de 100, Ótimo').waitForDisplayed({ timeout: 30_000 });
    const details = healthBlock().$('details');
    if (!(await details.getAttribute('open'))) {
      await healthBlock().$('summary=O que tirou pontos').click();
    }
    const ignored = healthBlock().$('ul.losses > li.is-ignored');
    await ignored.waitForDisplayed({ timeout: 10_000 });
    await expect(ignored).toHaveText(expect.stringContaining('(ignorado por você)'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d08-02-aviso-ignorado');
  });

  it('CA-T02-05: Meus packs mostra a mesma nota; pack sem diagnóstico mostra "Sem dados"', async () => {
    await backToPacks();
    const cell = healthCell('Vale Saudável');
    await browser.waitUntil(async () => (await cell.getText()).includes('90'), {
      timeout: 30_000,
      timeoutMsg: 'a coluna Saúde não mostrou a nota 90',
    });
    await expect(cell).toHaveText(expect.stringContaining('Ótimo'));
    const stored = registry().packs?.find((pack) => pack.path === PACK)?.health;
    expect(stored).toEqual({ score: 90, band: 'great' });

    // Um pack aberto e nunca verificado: sem nota.
    await openFolder(NEW_PACK);
    await backToPacks();
    const empty = healthCell('Pack Sem Nota');
    await empty.waitForDisplayed({ timeout: 30_000 });
    await expect(empty).toHaveText(expect.stringContaining('Sem dados'));
    await expect(healthCell('Vale Saudável')).toHaveText(expect.stringContaining('90'));
    // Abrir a lista não rodou o diagnóstico do pack novo.
    const newId = packId(NEW_PACK);
    const reports = existsSync(REPORTS_DIR) ? readdirSync(REPORTS_DIR) : [];
    expect(reports).not.toContain(`${newId}.json`);
    expect(registry().packs?.find((pack) => pack.id === newId)?.health).toBeUndefined();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d08-03-coluna-saude');
  });
});
