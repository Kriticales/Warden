/**
 * Instância pronta para o Prism no app real (E-04; SPEC T19, CA-T19-09). O pack de teste só tem
 * configs (nenhum mod): os mods do Modrinth e da CurseForge, os hashes e a troca de mods
 * bloqueados são provados no Rust (`crates/warden-export/tests/prism`) com servidores
 * simulados, porque o Prism baixa de `cdn.modrinth.com` e `edge.forgecdn.net` (https), que o
 * servidor de fixtures dos E2E não imita.
 *
 * - Pack sem `version`: o aviso aparece e Gerar instância fica desabilitado.
 * - Com `version`: o arquivo `.mrpack` gerado tem `modrinth.index.json` válido (formatVersion,
 *   dependências exatas do `pack.toml`), `overrides/` só com os arquivos do índice, e o SHA-256
 *   do resultado é o do arquivo gravado.
 * - Destino ocupado: a frase explica e nada é sobrescrito.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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
const BINARIES = join(import.meta.dirname, '..', 'src-tauri', 'binaries');
const PACK = join(DATA_ROOT, 'fora', 'pack-prism');
const OUT = join(DATA_ROOT, 'prism');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

function packwizBinary(): string {
  const suffix = process.platform === 'win32' ? 'windows-msvc.exe' : 'linux-gnu';
  const name = readdirSync(BINARIES).find(
    (entry) =>
      entry.startsWith('packwiz-') &&
      entry.endsWith(suffix) &&
      !entry.includes('.build') &&
      !entry.endsWith('.commit'),
  );
  if (!name) throw new Error('sidecar do packwiz ausente: rode cargo xtask build-packwiz');
  return join(BINARIES, name);
}

function writePack(version: string): void {
  rmSync(PACK, { recursive: true, force: true });
  mkdirSync(join(PACK, 'config'), { recursive: true });
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Pack do Prism"',
      ...(version ? [`version = "${version}"`] : []),
      'description = "Teste da instância pronta"',
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      'hash = ""',
      '',
      '[versions]',
      'fabric = "0.16.5"',
      'minecraft = "1.21.1"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, 'index.toml'), 'hash-format = "sha256"\n');
  writeFileSync(join(PACK, 'config', 'a.txt'), 'ajuste\n');
  execFileSync(packwizBinary(), ['refresh'], { cwd: PACK, stdio: 'pipe' });
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

/** Nomes das entradas de arquivo de um zip (diretório central). */
function zipEntries(path: string): string[] {
  const zip = readFileSync(path);
  let end = zip.length - 22;
  while (end >= 0 && zip.readUInt32LE(end) !== 0x06054b50) end -= 1;
  if (end < 0) throw new Error('zip sem diretório central');
  const count = zip.readUInt16LE(end + 10);
  let offset = zip.readUInt32LE(end + 16);
  const names: string[] = [];
  for (let i = 0; i < count; i += 1) {
    const nameLength = zip.readUInt16LE(offset + 28);
    const extra = zip.readUInt16LE(offset + 30);
    const comment = zip.readUInt16LE(offset + 32);
    names.push(zip.toString('utf8', offset + 46, offset + 46 + nameLength));
    offset += 46 + nameLength + extra + comment;
  }
  return names.sort();
}

/** Lê o `modrinth.index.json` de um `.mrpack` com o `unzip` do sistema (ou `tar` no Windows). */
function readIndex(path: string): Record<string, unknown> {
  const text =
    process.platform === 'win32'
      ? execFileSync('tar', ['-xOf', path, 'modrinth.index.json'], { encoding: 'utf8' })
      : execFileSync('unzip', ['-p', path, 'modrinth.index.json'], { encoding: 'utf8' });
  return JSON.parse(text) as Record<string, unknown>;
}

/** Se o passo falhar, o erro diz qual foi e o que a tela mostrava (o CI não guarda capturas). */
async function step(name: string, action: () => Promise<void>): Promise<void> {
  try {
    await action();
  } catch (error) {
    const body = await browser.execute(() =>
      (document.querySelector('main') ?? document.body).innerText.slice(0, 1200),
    );
    throw new Error(`passo "${name}" falhou: ${String(error)}\ntela: ${body}`);
  }
}

async function openPrismFormat(): Promise<void> {
  await $('nav[aria-label="Seções do pack"]').$('a*=Exportar').click();
  await $('h1=Exportar').waitForDisplayed();
  await $('h2=2. Formato').waitForDisplayed({ timeout: 60_000 });
  await $('label*=Instância pronta para o Prism').click();
}

describe('Instância pronta para o Prism (E-04; T19)', () => {
  before(async () => {
    rmSync(OUT, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    mkdirSync(OUT, { recursive: true });
    writePack('');
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('sem versão no pack: avisa e não deixa gerar', async () => {
    await step('lista vazia', async () => {
      await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 20_000 });
    });
    await step('abrir o pack', async () => {
      writeFileSync(PICK_FILE, PACK);
      await $('button=Abrir ou importar…').click();
      await $('h1=Abrir pack').waitForDisplayed({ timeout: 20_000 });
      await $('button=Abrir pack').click();
      await $('h1=Mods').waitForDisplayed({ timeout: 30_000 });
    });
    await step('formato Prism', async () => {
      await openPrismFormat();
      await expect($('main')).toHaveText(expect.stringContaining('O pack ainda não tem versão'), {
        wait: 30_000,
      });
    });
    await expect($('button=Gerar instância…')).toBeDisabled();
    await screenshot('40-prism-sem-versao');
  });

  it('com versão: gera o .mrpack, confere o conteúdo e o SHA-256 do resultado', async () => {
    writePack('1.2.0');
    await browser.refresh();
    await $('h1=Exportar').waitForDisplayed({ timeout: 60_000 });
    await $('label*=Instância pronta para o Prism').click();
    await $('h2=O que vai no arquivo').waitForDisplayed({ timeout: 60_000 });
    await expect($('main')).toHaveText(expect.stringContaining('Minecraft 1.21.1'));
    await screenshot('41-prism-analise');

    const out = join(OUT, 'pack-prism.mrpack');
    writeFileSync(PICK_FILE, out);
    await $('button=Gerar instância…').click();
    const dialog = $('[role="dialog"]*=Instância pronta para o Prism gerada');
    await dialog.waitForDisplayed({ timeout: 120_000 });
    await screenshot('42-prism-resultado');
    await expect(dialog).toHaveText(expect.stringContaining('Baixe o arquivo e arraste'));

    const bytes = readFileSync(out);
    const sha = createHash('sha256').update(bytes).digest('hex');
    await expect(dialog).toHaveText(expect.stringContaining(sha));

    const index = readIndex(out);
    expect(index.formatVersion).toBe(1);
    expect(index.versionId).toBe('1.2.0');
    expect(index.dependencies).toEqual({ minecraft: '1.21.1', 'fabric-loader': '0.16.5' });
    expect(zipEntries(out)).toEqual(['modrinth.index.json', 'overrides/config/a.txt']);
    await dialog.$('button=Fechar').click();
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });

  it('destino ocupado: explica o que fazer e não sobrescreve', async () => {
    const out = join(OUT, 'pack-prism.mrpack');
    const before = readFileSync(out);
    writeFileSync(PICK_FILE, out);
    // Um aviso ainda na tela pode cobrir o botão: o clique vai direto no elemento.
    await $('button=Gerar instância…').waitForClickable({ timeout: 30_000 });
    const element = await $('button=Gerar instância…');
    await browser.execute(
      (button: HTMLElement) => {
        button.click();
      },
      element as unknown as HTMLElement,
    );
    const panel = $('main .alert--danger');
    await panel.waitForDisplayed({ timeout: 60_000 });
    await expect(panel).toHaveText(expect.stringContaining('O destino escolhido já existe.'));
    expect(readFileSync(out).equals(before)).toBe(true);
    await screenshot('43-prism-destino-ocupado');
  });
});
