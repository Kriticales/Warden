/**
 * Adicionar pelo Modrinth no app real (P1-09; SPEC T08 e T09), com o packwiz de verdade e as
 * respostas gravadas do Modrinth (servidor de fixtures; nada vai à internet).
 *
 * O pack é o que o packwiz real gerou para o Fabric 1.21.1
 * (`crates/warden-packwiz/tests/fixtures/packwiz-output/fabric-1.21.1`), sem o Sodium.
 *
 * - Mods → Adicionar abre a página em tela cheia com o menu recolhido (ESTRUTURA N6).
 * - Buscar "sodium" traz o Sodium entre os 3 primeiros (CA-T08-01).
 * - A pré-visualização mostra a versão estável mais nova.
 * - Marcar 2 resultados abre um único diálogo de dependências (CA-T08-11); um deles não tem
 *   versão para o pack e fica de fora.
 * - Confirmar grava `mods/sodium.pw.toml` com `[update.modrinth]`, `sha512` e `side = "client"`,
 *   o índice passa a listá-lo e a busca continua aberta, com "Já no pack".
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-adicionar');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

function pickNext(folder: string): void {
  writeFileSync(PICK_FILE, folder);
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

/** Tira o Sodium do pack copiado, como se ele nunca tivesse entrado (índice e hash dele). */
function removeSodium(): void {
  rmSync(join(PACK, 'mods', 'sodium.pw.toml'));
  const index = read('index.toml').replace(
    /\[\[files\]\]\nfile = "mods\/sodium\.pw\.toml"\nhash = "[0-9a-f]+"\nmetafile = true\n\n/,
    '',
  );
  expect(index).not.toContain('sodium');
  writeFileSync(join(PACK, 'index.toml'), index);
  const hash = createHash('sha256').update(index).digest('hex');
  writeFileSync(
    join(PACK, 'pack.toml'),
    read('pack.toml').replace(/^hash = "[0-9a-f]+"$/m, `hash = "${hash}"`),
  );
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

/** Texto completo de um elemento (inclusive opções de seletores e o que está fora da tela). */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** Os nomes dos resultados, na ordem. */
async function resultNames(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('.drow .drow__name')).map((b) => b.textContent),
  );
}

describe('Adicionar pelo Modrinth (P1-09)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    removeSodium();
    pickNext('');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados vazia para os testes que rodam depois (inclusive o cache de
    // metadados do Modrinth, que mudaria o que os detalhes do editor mostram).
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
  });

  it('Mods → Adicionar abre a página em tela cheia, com o menu recolhido', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Fixture Fabric').waitForDisplayed({ timeout: 60_000 });
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });

    await $('a=Adicionar').click();
    await $('h1=Adicionar ao pack').waitForDisplayed();
    expect(await $('.secmenu--compact').isExisting()).toBe(true);
    expect(await $('body').getText()).toContain('Minecraft 1.21.1 com Fabric');
  });

  it('buscar "sodium" traz o Sodium entre os 3 primeiros (CA-T08-01)', async () => {
    const field = $('input[type="search"]');
    await field.setValue('sodium');
    await $('button.drow__name=Sodium').waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(async () => (await resultNames()).length >= 3);
    const names = await resultNames();
    expect(names.slice(0, 3)).toContain('Sodium');
    expect(await seriousViolations()).toEqual([]);
    await screenshot('01-busca');
  });

  it('a pré-visualização mostra a versão estável mais nova', async () => {
    await $('button.drow__name=Sodium').click();
    const preview = $('aside.disc__preview');
    await preview.waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(
      async () => (await fullText('aside.disc__preview')).includes('(mais nova compatível)'),
      { timeout: 30_000, timeoutMsg: 'seletor de versão sem a versão padrão' },
    );
    expect(await fullText('aside.disc__preview')).toContain('mc1.21.1-0.8.13-fabric');
    await screenshot('02-previa');
  });

  it('marcar 2 resultados abre um único diálogo de dependências (CA-T08-11)', async () => {
    await $('input[aria-label="Selecionar Sodium"]').click();
    await $('input[aria-label="Selecionar Sodium Extra"]').click();
    await $('.selbar*=2 selecionados').waitForDisplayed();
    await screenshot('03-selecao');
    await $('button=Adicionar 2 ao pack').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await browser.waitUntil(
      async () => (await fullText('[role="dialog"]')).includes('O que você escolheu'),
      { timeout: 30_000, timeoutMsg: 'o plano não apareceu no diálogo' },
    );
    const text = await fullText('[role="dialog"]');
    expect(text).toContain('Sodium');
    // O Sodium Extra não tem versão gravada para o pack: fica de fora, com o motivo.
    expect(text).toContain('Sem versão compatível');
    expect(await $$('[role="dialog"]').length).toBe(1);
    await screenshot('04-dependencias');
  });

  it('confirmar grava o .pw.toml do Modrinth e a busca continua aberta', async () => {
    await $('[role="dialog"]').$('button=Adicionar 1 item').click();
    await waitToast('1 item adicionado ao pack');
    await $('[role="dialog"]').waitForExist({ reverse: true });

    const sodium = read('mods/sodium.pw.toml');
    expect(sodium).toContain('[update.modrinth]');
    expect(sodium).toContain('mod-id = "AANobbMI"');
    expect(sodium).toContain('hash-format = "sha512"');
    expect(sodium).toContain('side = "client"');
    expect(read('index.toml')).toContain('mods/sodium.pw.toml');

    await $('h1=Adicionar ao pack').waitForDisplayed();
    await $('.drow*=Já no pack').waitForDisplayed({ timeout: 30_000 });
    expect(await $('input[aria-label="Selecionar Sodium"]').isExisting()).toBe(false);
    expect(await $('body').getText()).toContain('Voltar para Mods · 1 adicionado');
    await screenshot('05-adicionado');
  });

  it('voltar para Mods mostra o Sodium na lista', async () => {
    await $('a*=Voltar para Mods').click();
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Sodium').waitForDisplayed({ timeout: 30_000 });
  });
});
