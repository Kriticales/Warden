/**
 * Formatos de outros launchers no app real (E-02; SPEC T19; CA-T19-04 e CA-T19-05), com o
 * packwiz de verdade. O pack é o do fixture do packwiz sem as referências e com um `.jar`
 * local no índice: assim a geração não precisa de internet (os mods por referência e as
 * trocas pelo Modrinth são provados no teste de integração do Rust com servidores simulados).
 *
 * - `.mrpack`: a tela explica o que se perde; sem `version` no `pack.toml` pede a versão; o
 *   `.jar` local pede a confirmação de licença; o arquivo gerado tem `modrinth.index.json`
 *   válido (sem `env` fora da especificação) e o `.jar` só em `overrides/`.
 * - Zip da CurseForge: `manifest.json` válido e o `.jar` em `overrides/`, só depois da
 *   confirmação.
 * - O `pack.toml` do pack no Warden continua sem `version` (só o arquivo gerado a leva).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { inflateRawSync } from 'node:zlib';

import { $, browser, expect } from '@wdio/globals';

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
const BINARIES = join(import.meta.dirname, '..', 'src-tauri', 'binaries');
const PACK = join(DATA_ROOT, 'fora', 'pack-formatos');
const OUT = join(DATA_ROOT, 'exportados-formatos');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const LOCAL_JAR = 'mods/mod-local.jar';

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.execute(() => {
    for (const close of document.querySelectorAll<HTMLButtonElement>('.toast button:last-child')) {
      close.click();
    }
  });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function pickNext(path: string): void {
  writeFileSync(PICK_FILE, path);
}

function packwizBinary(): string {
  const name =
    process.platform === 'win32'
      ? 'packwiz-x86_64-pc-windows-msvc.exe'
      : 'packwiz-x86_64-unknown-linux-gnu';
  const path = join(BINARIES, name);
  if (!existsSync(path)) throw new Error('sidecar do packwiz ausente: rode cargo xtask setup');
  return path;
}

/** O conteúdo de uma entrada de um zip (diretório central; `stored` ou `deflate`). */
function zipEntry(path: string, name: string): Buffer | null {
  const zip = readFileSync(path);
  let end = zip.length - 22;
  while (end >= 0 && zip.readUInt32LE(end) !== 0x06054b50) end -= 1;
  if (end < 0) throw new Error('zip sem diretório central');
  const count = zip.readUInt16LE(end + 10);
  let offset = zip.readUInt32LE(end + 16);
  for (let i = 0; i < count; i += 1) {
    expect(zip.readUInt32LE(offset)).toBe(0x02014b50);
    const method = zip.readUInt16LE(offset + 10);
    const size = zip.readUInt32LE(offset + 20);
    const nameLength = zip.readUInt16LE(offset + 28);
    const extra = zip.readUInt16LE(offset + 30);
    const comment = zip.readUInt16LE(offset + 32);
    const local = zip.readUInt32LE(offset + 42);
    const entry = zip.toString('utf8', offset + 46, offset + 46 + nameLength);
    if (entry === name) {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return method === 0 ? Buffer.from(data) : inflateRawSync(data);
    }
    offset += 46 + nameLength + extra + comment;
  }
  return null;
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

/** Clica em Exportar… e espera o diálogo do resultado. */
async function exportAndWait(): Promise<void> {
  await $('button=Exportar…').click();
  const dialog = $('[role="dialog"]*=Arquivo exportado');
  let error = '';
  await browser.waitUntil(
    async () => {
      if (await dialog.isExisting()) return true;
      const panel = $('main .alert--danger');
      if (await panel.isExisting()) error = await panel.getText();
      return error !== '';
    },
    { timeout: 120_000, timeoutMsg: 'a exportação não terminou' },
  );
  expect(error).toBe('');
}

async function closeDialog(): Promise<void> {
  await $('[role="dialog"]').$('button=Fechar').click();
  await $('[role="dialog"]').waitForExist({ reverse: true });
}

describe('Exportar para outros launchers (E-02; T19)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    rmSync(OUT, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    mkdirSync(OUT, { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    // Sem referências (nada para baixar) e sem `version`; com um jar local no índice.
    for (const folder of ['mods', 'resourcepacks', 'shaderpacks']) {
      for (const entry of readdirSync(join(PACK, folder))) {
        if (entry.endsWith('.pw.toml')) rmSync(join(PACK, folder, entry));
      }
    }
    writeFileSync(join(PACK, LOCAL_JAR), 'jar local de teste, não é um mod de verdade\n');
    const manifest = readFileSync(join(PACK, 'pack.toml'), 'utf8').replace(/^version = .*\n/m, '');
    writeFileSync(join(PACK, 'pack.toml'), manifest);
    execFileSync(packwizBinary(), ['refresh'], { cwd: PACK, stdio: 'pipe' });
    pickNext('');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados vazia para os testes que rodam depois.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('.mrpack: explica o que se perde, pede a versão e a confirmação e só então exporta; axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
    await $('nav[aria-label="Seções do pack"]').$('a*=Exportar').click();
    await $('h1=Exportar').waitForDisplayed();
    await $('h2=3. O que vai no pack').waitForDisplayed({ timeout: 60_000 });

    await $('label*=.mrpack').click();
    await $('h2=O que este formato perde').waitForDisplayed({ timeout: 60_000 });
    const main = $('main');
    await expect(main).toHaveText(
      expect.stringContaining('cada versão nova é um arquivo novo para baixar e importar'),
    );
    await expect(main).toHaveText(expect.stringContaining('1 arquivo de terceiros vai dentro'));
    await expect(main).toHaveText(expect.stringContaining('Versão do pack'));
    await screenshot('40-exportar-mrpack');
    expect(await seriousViolations()).toEqual([]);

    // Sem versão e sem a confirmação, Exportar… fica travado.
    await expect($('button=Exportar…')).toBeDisabled();
    await $('section[aria-labelledby="exf-versao"] input').setValue('1.0.0');
    await expect($('button=Exportar…')).toBeDisabled();
    await expect(main).toHaveText(
      expect.stringContaining('Confirme que pode incluir os arquivos de terceiros'),
    );
    await $('label*=Tenho permissão para incluir estes arquivos').click();
    await expect($('button=Exportar…')).toBeEnabled();

    const file = join(OUT, 'pack.mrpack');
    pickNext(file);
    await exportAndWait();
    await expect($('[role="dialog"]')).toHaveText(expect.stringContaining('Arquivo conferido.'));
    await screenshot('41-mrpack-exportado');
    await closeDialog();

    // CA-T19-04: índice válido, jar só em overrides/, nenhum `env` fora da especificação.
    const index = JSON.parse(zipEntry(file, 'modrinth.index.json')?.toString('utf8') ?? 'null') as {
      formatVersion: number;
      game: string;
      versionId: string;
      dependencies: Record<string, string>;
      files: { env?: Record<string, string> }[];
    };
    expect(index.formatVersion).toBe(1);
    expect(index.game).toBe('minecraft');
    expect(index.versionId).toBe('1.0.0');
    expect(index.dependencies.minecraft).toBe('1.21.1');
    expect(index.dependencies['fabric-loader']).toBe('0.16.14');
    for (const entry of index.files) {
      for (const value of Object.values(entry.env ?? {})) {
        expect(['required', 'optional', 'unsupported']).toContain(value);
      }
    }
    expect(zipEntry(file, `overrides/${LOCAL_JAR}`)).not.toBeNull();
    expect(zipEntry(file, 'overrides/config/exemplo.properties')).not.toBeNull();
    // O pack no Warden não ganhou `version`.
    expect(readFileSync(join(PACK, 'pack.toml'), 'utf8')).not.toMatch(/^version = /m);
  });

  it('zip da CurseForge: manifest.json válido e o jar local em overrides/, depois da confirmação', async () => {
    await $('label*=.zip da CurseForge').click();
    await $('h2=O que este formato perde').waitForDisplayed({ timeout: 60_000 });
    const main = $('main');
    await expect(main).toHaveText(
      expect.stringContaining('A CurseForge exige aprovação manual de arquivos que não são dela'),
    );
    await $('section[aria-labelledby="exf-versao"] input').setValue('1.0.0');
    await expect($('button=Exportar…')).toBeDisabled();
    await $('label*=Tenho permissão para incluir estes arquivos').click();
    await screenshot('42-exportar-curseforge');

    const file = join(OUT, 'pack-curseforge.zip');
    pickNext(file);
    await exportAndWait();
    await closeDialog();

    // CA-T19-05: manifest.json válido; nenhum jar sem a confirmação (aqui, com ela).
    const manifest = JSON.parse(zipEntry(file, 'manifest.json')?.toString('utf8') ?? 'null') as {
      manifestType: string;
      manifestVersion: number;
      version: string;
      overrides: string;
      minecraft: { version: string; modLoaders: { id: string; primary: boolean }[] };
    };
    expect(manifest.manifestType).toBe('minecraftModpack');
    expect(manifest.manifestVersion).toBe(1);
    expect(manifest.version).toBe('1.0.0');
    expect(manifest.minecraft.version).toBe('1.21.1');
    expect(manifest.minecraft.modLoaders.some((loader) => loader.id.startsWith('fabric-'))).toBe(
      true,
    );
    expect(zipEntry(file, `${manifest.overrides}/${LOCAL_JAR}`)).not.toBeNull();
    expect(existsSync(file)).toBe(true);
  });

  it('destino ocupado: explica e não grava nada', async () => {
    const file = join(OUT, 'pack-curseforge.zip');
    const before = readFileSync(file);
    pickNext(file);
    await $('button=Exportar…').click();
    const panel = $('main .alert--danger');
    await panel.waitForDisplayed({ timeout: 30_000 });
    await expect(panel).toHaveText(expect.stringContaining('O destino escolhido já existe.'));
    expect(readFileSync(file).equals(before)).toBe(true);
  });
});
