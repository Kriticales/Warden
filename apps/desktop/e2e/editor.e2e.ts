/**
 * Pack aberto no app real (P1-08; SPEC T05, T06, T07 e T11), com o packwiz de verdade e as
 * respostas gravadas do Modrinth (servidor de fixtures; nada vai à internet).
 *
 * O pack é o que o packwiz real gerou para o Fabric 1.21.1
 * (`crates/warden-packwiz/tests/fixtures/packwiz-output/fabric-1.21.1`): mods do Modrinth, da
 * CurseForge e de link direto, um resource pack e um shader.
 *
 * - Abre em Mods com o cabeçalho e as 6 seções (CA-T05-03); versão legível, nunca um ID.
 * - Lado na linha grava só a linha `side` do `.pw.toml` (CA-T06-03, um item).
 * - Detalhes do Modrinth (respostas gravadas) e da CurseForge sem chave (CA-T07-02).
 * - Informações do pack: mudar o nome muda só a linha `name` do `pack.toml` (CA-T11-01).
 * - Ajustes do teste: Java automático com o motivo; nada no pack muda.
 * - Remover apaga o `.pw.toml` e a entrada do índice (CA-T06-04).
 * - Um `.pw.toml` quebrado e um jar fora do índice aparecem como linhas com problema.
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
  statSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join, relative } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-fabric');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  // Os avisos (toasts) das etapas anteriores cobririam a tela: saem antes da captura.
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function pickNext(folder: string): void {
  writeFileSync(PICK_FILE, folder);
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

/** As linhas que mudaram entre dois textos com o mesmo número de linhas. */
function changedLines(before: string, after: string): string[] {
  const a = before.split('\n');
  const b = after.split('\n');
  expect(b.length).toBe(a.length);
  return b.filter((line, index) => line !== a[index]);
}

/** Hash de todos os arquivos do pack (caminho + conteúdo), sem a pasta do git. */
function folderHash(dir: string): string {
  const hash = createHash('sha256');
  const walk = (current: string) => {
    for (const entry of readdirSync(current).sort()) {
      if (entry === '.git') continue;
      const path = join(current, entry);
      if (statSync(path).isDirectory()) {
        walk(path);
      } else {
        hash.update(relative(dir, path));
        hash.update(readFileSync(path));
      }
    }
  };
  walk(dir);
  return hash.digest('hex');
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

/** A linha da lista de Mods de um item, pelo nome. */
function modRow(name: string) {
  return $(`tr.modrow*=${name}`);
}

/** Espera um aviso (toast) e confere o texto; um aviso de erro falha na hora, com a frase. */
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

/** Texto completo de um elemento (inclusive o que está fora da área visível do painel). */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** Fecha o painel ou diálogo aberto, se houver. */
async function closeOverlay(): Promise<void> {
  if (await $('[role="dialog"]').isExisting()) {
    await browser.keys('Escape');
    await $('[role="dialog"]').waitForExist({ reverse: true });
  }
}

describe('Pack aberto: cabeçalho, Mods, detalhes e informações (P1-08)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    pickNext('');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados vazia para o packs.e2e.ts, que roda depois.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('abre o pack em Mods, com o cabeçalho e exatamente as 6 seções (CA-T05-03)', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();

    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('h1=Mods').waitForDisplayed();
    await modRow('Sodium').waitForDisplayed();
    const header = $('header.packhead');
    await expect(header).toHaveText(expect.stringContaining('Minecraft 1.21.1'));
    await expect(header).toHaveText(expect.stringContaining('Fabric 0.16.14'));
    await expect(header.$('a*=Meus packs')).toExist();
    const names = await $$('nav[aria-label="Seções do pack"] .secmenu__name').map((el) =>
      el.getText(),
    );
    expect(names).toEqual([
      'Mods',
      'Configs',
      'Problemas',
      'Diagnóstico com IA',
      'Histórico',
      'Exportar',
    ]);
    await expect($('h1=Mods')).toExist();
    await screenshot('20-pack-aberto-mods');
    expect(await seriousViolations()).toEqual([]);
  });

  it('versão legível pelo cache do Modrinth e pelo arquivo; nunca um ID (CA-T06-02)', async () => {
    // Sodium: número da versão das respostas gravadas do Modrinth.
    await expect(modRow('Sodium')).toHaveText(expect.stringContaining('mc1.21.1-0.8.13-fabric'));
    await expect(modRow('Jade')).toHaveText(expect.stringContaining('CurseForge'));
    await expect(modRow('Text Placeholder API')).toHaveText(expect.stringContaining('Link direto'));
    const text = await $('main').getText();
    for (const id of ['SMxNOGZ6', 'AANobbMI', '8591528', '324717']) {
      expect(text).not.toContain(id);
    }
    await expect($('button*=Resource packs')).toExist();
    await expect($('button*=Shaders')).toExist();
  });

  it('lado na linha muda só a linha side do .pw.toml (CA-T06-03, um item)', async () => {
    const before = read('mods/lithium.pw.toml');
    const select = $('select[aria-label="Lado de Lithium"]');
    const current = await select.getValue();
    const next = current === 'client' ? 'server' : 'client';
    await select.selectByAttribute('value', next);
    await waitToast('Concluída: Alterar lado');
    const after = read('mods/lithium.pw.toml');
    expect(changedLines(before, after)).toEqual([`side = "${next}"`]);
  });

  it('detalhes do Modrinth num painel lateral, sem sair da lista', async () => {
    await closeOverlay();
    await modRow('Sodium').$('button=Sodium').click();
    const drawer = $('[role="dialog"]');
    await drawer.waitForDisplayed();
    await browser.waitUntil(async () =>
      (await fullText('[role="dialog"]')).includes('mc1.21.1-0.8.13-fabric'),
    );
    const text = await fullText('[role="dialog"]');
    expect(text).toContain('Detalhes do mod');
    expect(text).toContain('sodium-fabric-0.8.13+mc1.21.1.jar');
    await screenshot('21-detalhes-modrinth');
    expect(await seriousViolations()).toEqual([]);
    await closeOverlay();
  });

  it('CurseForge sem chave: avisa e ainda mostra nome, arquivo, lado e hash (CA-T07-02)', async () => {
    await modRow('Jade').$('button*=Jade').click();
    const drawer = $('[role="dialog"]');
    await drawer.waitForDisplayed();
    await browser.waitUntil(async () =>
      (await fullText('[role="dialog"]')).includes('Detalhes da CurseForge precisam da chave'),
    );
    const metafile = read('mods/jade.pw.toml');
    const filename = /filename = "(.+)"/.exec(metafile)?.[1] ?? '';
    const hash = /hash = "(.+)"/.exec(metafile)?.[1] ?? '';
    const text = await fullText('[role="dialog"]');
    expect(text).toContain(filename);
    expect(text).toContain(hash);
    await screenshot('22-detalhes-curseforge-sem-chave');
    await closeOverlay();
  });

  it('Informações do pack: mudar o nome muda só a linha name do pack.toml (CA-T11-01)', async () => {
    await closeOverlay();
    const before = read('pack.toml');
    await $('header.packhead').$('button*=Editar informações').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    const nameId = await dialog.$('label=Nome').getAttribute('for');
    const name = $(`[id="${nameId ?? ''}"]`);
    await browser.waitUntil(async () => (await name.getValue()) === 'Fixture Fabric');
    await screenshot('23-informacoes-do-pack');
    expect(await seriousViolations()).toEqual([]);
    await name.setValue('Vale Fabric');
    await dialog.$('button=Salvar').click();
    await $('h1=Vale Fabric').waitForDisplayed({ timeout: 30_000 });
    expect(changedLines(before, read('pack.toml'))).toEqual(['name = "Vale Fabric"']);
  });

  it('Ajustes do teste: Java automático com o motivo; grava em packs.json e o pack não muda', async () => {
    const before = folderHash(PACK);
    await $('header.packhead').$('button*=Mais opções do teste').click();
    await $('[role="menu"]').$('div*=Ajustes do teste neste computador').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await dialog.$('option=Automático: Java 21').waitForExist({ timeout: 30_000 });
    await expect(dialog).toHaveText(expect.stringContaining('Por que não o Java 25?'));
    await screenshot('24-ajustes-do-teste');
    expect(await seriousViolations()).toEqual([]);
    const memoryId = await dialog.$('label=Memória do teste').getAttribute('for');
    await $(`[id="${memoryId ?? ''}"]`).selectByAttribute('value', '6144');
    await dialog.$('button=Salvar ajustes').click();
    await waitToast('Ajustes do teste salvos.');
    expect(readFileSync(REGISTRY_FILE, 'utf8')).toContain('"testSettings"');
    expect(folderHash(PACK)).toBe(before);
  });

  it('Remover apaga o .pw.toml e a entrada do índice (CA-T06-04)', async () => {
    await $('input[aria-label="Selecionar Mod Menu"]').click();
    await $('[aria-label="Ações para os selecionados"]').$('button*=Remover').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('Remover Mod Menu?'));
    await browser.waitUntil(async () =>
      (await fullText('[role="alertdialog"]')).includes('depende disso'),
    );
    await screenshot('25-remover');
    await dialog.$('button=Remover Mod Menu').click();
    await waitToast('Concluída: Remover do pack');
    expect(existsSync(join(PACK, 'mods', 'modmenu.pw.toml'))).toBe(false);
    expect(read('index.toml')).not.toContain('modmenu');
    await expect(modRow('Mod Menu')).not.toExist();
  });

  it('arquivo quebrado e jar fora do índice viram linhas com problema; o resto segue (CA-T06-01)', async () => {
    writeFileSync(join(PACK, 'mods', 'quebrado.pw.toml'), 'name = "Quebrado\nfilename = ');
    const index = read('index.toml');
    writeFileSync(
      join(PACK, 'index.toml'),
      `${index}\n[[files]]\nfile = "mods/quebrado.pw.toml"\nhash = "${'0'.repeat(64)}"\nmetafile = true\n`,
    );
    writeFileSync(join(PACK, 'mods', 'solto-1.0.jar'), 'não é um jar de verdade');
    await browser.refresh();
    await $('h1=Mods').waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(
      async () => (await fullText('main')).includes('1 arquivo do pack não pôde ser lido.'),
      { timeout: 30_000 },
    );
    await expect(modRow('quebrado.pw.toml')).toHaveText(expect.stringMatching(/arquivo inválido/i));
    await expect(modRow('solto-1.0.jar')).toHaveText(expect.stringMatching(/fora do índice/i));
    await expect(modRow('Sodium')).toExist();
    await screenshot('26-mods-com-problemas');
    expect(await seriousViolations()).toEqual([]);
  });

  it('em 1024 px (largura mínima) o pack aberto não estoura na horizontal', async () => {
    await browser.setWindowSize(1024, 700);
    await browser.pause(300);
    const overflow = await browser.execute(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBe(0);
    await screenshot('27-pack-1024px');
    await browser.setWindowSize(1366, 860);
  });
});
