/**
 * Seção Exportar no app real (E-01; SPEC T19), com o packwiz de verdade. O pack é o que o
 * packwiz real gerou para o Fabric 1.21.1 (`crates/warden-packwiz/tests/fixtures/packwiz-output/
 * fabric-1.21.1`): mods, um resource pack, um shader, uma config e um `options.txt` no índice.
 *
 * - A prévia mostra o que vai e alerta o `options.txt` solto na raiz; com `preserve = true` no
 *   índice, ele não é apontado como "substitui as preferências".
 * - CA-T19-03: um `crash-reports/x.txt` criado à mão aparece na higiene e, depois de Limpar,
 *   não está mais no disco nem na exportação.
 * - CA-T19-01: a pasta exportada tem exatamente `pack.toml`, `index.toml` e os arquivos do
 *   índice; o `.zip` tem as mesmas entradas.
 * - CA-T19-02 (parte do app): `packwiz refresh` na pasta exportada não muda nada. A instalação
 *   pelo packwiz-installer real fica no teste de conformidade do Rust (`warden-export`).
 * - Destino ocupado: a frase explica o que fazer e nada é gravado.
 * - Excluir do pack: a regra entra no `.packwizignore` e o arquivo sai do índice.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join, relative } from 'node:path';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-exportar');
const OUT = join(DATA_ROOT, 'exportados');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

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

/** Os arquivos de uma pasta (caminhos com `/`), em ordem. */
function listFiles(dir: string): string[] {
  const out: string[] = [];
  const walk = (current: string) => {
    for (const entry of readdirSync(current)) {
      const path = join(current, entry);
      if (statSync(path).isDirectory()) walk(path);
      else out.push(relative(dir, path).replaceAll('\\', '/'));
    }
  };
  walk(dir);
  return out.sort();
}

function folderHash(dir: string): string {
  const hash = createHash('sha256');
  for (const path of listFiles(dir)) {
    hash.update(path);
    hash.update(readFileSync(join(dir, path)));
  }
  return hash.digest('hex');
}

/** `pack.toml`, `index.toml` e os `file = "…"` do índice do pack. */
function expectedFiles(pack: string): string[] {
  const index = readFileSync(join(pack, 'index.toml'), 'utf8');
  const files = [...index.matchAll(/^file = "([^"]+)"/gm)].map((match) => match[1] ?? '');
  return ['index.toml', 'pack.toml', ...files].sort();
}

/** Nomes das entradas de arquivo de um zip (diretório central; sem as entradas de pasta). */
function zipEntries(path: string): string[] {
  const zip = readFileSync(path);
  let end = zip.length - 22;
  while (end >= 0 && zip.readUInt32LE(end) !== 0x06054b50) end -= 1;
  if (end < 0) throw new Error('zip sem diretório central');
  const count = zip.readUInt16LE(end + 10);
  let offset = zip.readUInt32LE(end + 16);
  const names: string[] = [];
  for (let i = 0; i < count; i += 1) {
    expect(zip.readUInt32LE(offset)).toBe(0x02014b50);
    const nameLength = zip.readUInt16LE(offset + 28);
    const extra = zip.readUInt16LE(offset + 30);
    const comment = zip.readUInt16LE(offset + 32);
    names.push(zip.toString('utf8', offset + 46, offset + 46 + nameLength));
    offset += 46 + nameLength + extra + comment;
  }
  return names.filter((name) => !name.endsWith('/')).sort();
}

function packwizBinary(): string {
  const name = readdirSync(BINARIES).find(
    (entry) =>
      entry.startsWith('packwiz-') && !entry.includes('.build') && !entry.endsWith('.commit'),
  );
  if (!name) throw new Error('sidecar do packwiz ausente: rode cargo xtask build-packwiz');
  return join(BINARIES, name);
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

async function waitToast(text: string): Promise<void> {
  let seen = '';
  await browser.waitUntil(
    async () => {
      seen = await browser.execute(() =>
        Array.from(document.querySelectorAll('.toast'))
          .map((toast) => toast.textContent)
          .join(' | '),
      );
      return seen.includes(text) || seen.includes('Não foi possível') || /falhou/i.test(seen);
    },
    { timeout: 60_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
  expect(seen).toContain(text);
}

/** Clica em Exportar… e espera o diálogo do resultado. */
async function exportAndWait(): Promise<void> {
  await $('button=Exportar…').click();
  const dialog = $('[role="dialog"]*=Pack exportado');
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

describe('Exportar (E-01; T19)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    rmSync(OUT, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    mkdirSync(OUT, { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
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

  it('abre a seção Exportar com a prévia e o alerta do options.txt solto na raiz; axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });

    // CA-T19-03: o arquivo de travamento é criado à mão, com o pack já aberto.
    mkdirSync(join(PACK, 'crash-reports'), { recursive: true });
    writeFileSync(join(PACK, 'crash-reports', 'x.txt'), 'travou\n');

    await $('nav[aria-label="Seções do pack"]').$('a*=Exportar').click();
    await $('h1=Exportar').waitForDisplayed();
    await $('h2=3. O que vai no pack').waitForDisplayed({ timeout: 60_000 });

    const main = $('main');
    await expect(main).toHaveText(
      expect.stringContaining('1 arquivo na pasta do pack não deveria ir para quem joga.'),
    );
    await expect(main).toHaveText(expect.stringContaining('1 arquivo pede atenção'));
    // Mesma conta do cabeçalho ("N alterações"): o pack aberto ainda não tem versão salva.
    await expect($('header.packhead')).toHaveText(expect.stringMatching(/\d+\s+alterações/));
    await expect(main).toHaveText(expect.stringContaining('Há alterações não salvas'));
    // O options.txt do fixture tem `preserve = true`: não substitui as preferências.
    await expect(main).not.toHaveText(expect.stringContaining('Substitui as preferências'));
    await expect(main).toHaveText(expect.stringContaining('Arquivo solto na raiz do pack'));
    await expect(main).toHaveText(
      expect.stringContaining('5 referências (os .jar não vão: quem joga baixa)'),
    );
    await screenshot('30-exportar');
    expect(await seriousViolations()).toEqual([]);
  });

  it('CA-T19-03: Revisar e limpar tira o crash-reports/x.txt', async () => {
    await $('button=Revisar e limpar').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    // A higiene aponta a pasta inteira de travamentos.
    await expect(dialog).toHaveText(expect.stringContaining('crash-reports'));
    await screenshot('31-exportar-limpar');
    await dialog.$('button=Limpar 1 arquivo').click();
    await waitToast('1 arquivo limpo');
    // O arquivo sai do disco. A pasta `crash-reports/` fica vazia e a varredura de higiene
    // (warden-project, P1-07) ainda a aponta: registrado no relatório da E-01. Vazia, ela não
    // vai na exportação (conferido no teste seguinte).
    expect(existsSync(join(PACK, 'crash-reports', 'x.txt'))).toBe(false);
  });

  it('CA-T19-01 e CA-T19-02: a pasta exportada é exatamente o índice e o packwiz não muda nada', async () => {
    const out = join(OUT, 'pasta');
    pickNext(out);
    await exportAndWait();
    const dialog = $('[role="dialog"]');
    await expect(dialog).toHaveText(expect.stringContaining('Conferido com o packwiz.'));
    await expect(dialog).toHaveText(expect.stringContaining(out));
    await screenshot('32-exportado');
    await closeDialog();

    expect(listFiles(out)).toEqual(expectedFiles(PACK));
    expect(listFiles(out).some((path) => path.startsWith('crash-reports'))).toBe(false);
    const before = folderHash(out);
    execFileSync(packwizBinary(), ['refresh'], { cwd: out, stdio: 'pipe' });
    expect(folderHash(out)).toBe(before);
  });

  it('CA-T19-01: o .zip tem as mesmas entradas', async () => {
    const zip = join(OUT, 'pack.zip');
    await $('label*=Arquivo .zip do pack packwiz').click();
    await expect($('main')).toHaveText(
      expect.stringContaining('A seguir, você escolhe onde salvar o arquivo .zip.'),
    );
    pickNext(zip);
    await exportAndWait();
    await expect($('[role="dialog"]')).toHaveText(expect.stringContaining('Mostrar o arquivo'));
    await closeDialog();
    expect(zipEntries(zip)).toEqual(expectedFiles(PACK));
  });

  it('destino ocupado: explica o que fazer e não grava nada', async () => {
    const zip = join(OUT, 'pack.zip');
    const before = readFileSync(zip);
    pickNext(zip);
    await $('button=Exportar…').click();
    const panel = $('main .alert--danger');
    await panel.waitForDisplayed({ timeout: 30_000 });
    await expect(panel).toHaveText(expect.stringContaining('O destino escolhido já existe.'));
    await screenshot('33-exportar-destino-ocupado');
    expect(readFileSync(zip).equals(before)).toBe(true);
  });

  it('Excluir do pack grava a regra no .packwizignore e tira o arquivo do índice', async () => {
    await $('summary*=mods/').click();
    await $('//button[.//span[text()="Excluir do pack: mods/modmenu.pw.toml"]]').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('/mods/modmenu.pw.toml'));
    await screenshot('34-excluir-do-pack');
    await dialog.$('button=Excluir do pack').click();
    await waitToast('Excluído do pack: mods/modmenu.pw.toml');

    const ignore = readFileSync(join(PACK, '.packwizignore'), 'utf8');
    expect(ignore.split(/\r?\n/)).toContain('/mods/modmenu.pw.toml');
    expect(readFileSync(join(PACK, 'index.toml'), 'utf8')).not.toContain('mods/modmenu.pw.toml');
    await expect($('main')).toHaveText(
      expect.stringContaining('4 referências (os .jar não vão: quem joga baixa)'),
    );
  });
});
