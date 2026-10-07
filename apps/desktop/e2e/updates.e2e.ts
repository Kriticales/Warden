/**
 * Atualizações no app real (P1-12; SPEC T10), com o packwiz de verdade e o servidor de fixtures
 * (`mock-server/fixtures/modrinth/v2/version_files*`: nada vai à internet).
 *
 * O pack é o do Fabric 1.21.1 (`packwiz-output/fabric-1.21.1`). No servidor simulado, o Lithium
 * o Mod Menu e o Fresh Animations têm versão nova; o Sodium está fixado; os outros itens do
 * Modrinth estão em dia.
 *
 * - Verificar atualizações mostra "3 atualizações disponíveis", só nos itens certos; o fixado,
 *   o link direto e a CurseForge (sem chave) não ganham botão Atualizar (CA-T10-02).
 * - Atualizar um item mostra a versão de agora e a nova, as novidades, e grava só a referência
 *   no `.pw.toml` (lado e nome ficam), com o índice atualizado.
 * - Revisar e atualizar (um item desmarcado) cria o ponto de segurança antes e depois a
 *   faixa some; nada some do pack.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

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
const PACK = join(DATA_ROOT, 'fora', 'pack-atualizacoes');
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

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8').replace(/\r\n/g, '\n');
}

/** A linha da lista de Mods de um item, pelo nome. */
function modRow(name: string) {
  return $(`tr.modrow*=${name}`);
}

async function seriousViolations(): Promise<string[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async () => {
    const { axe } = window as unknown as {
      axe: {
        run: (
          options: object,
        ) => Promise<{ violations: { id: string; impact: string | null; help: string }[] }>;
      };
    };
    try {
      return (await axe.run({ resultTypes: ['violations'] })).violations;
    } catch {
      return [{ id: 'axe-falhou', impact: 'critical', help: 'o axe não rodou' }];
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
      return seen.includes(text) || seen.includes('Não foi possível') || /erro/i.test(seen);
    },
    { timeout: 45_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
  expect(seen).toContain(text);
}

async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** Pontos de segurança gravados no repositório do pack. */
function safetyPoints(): string[] {
  const dir = join(PACK, '.git', 'refs', 'warden', 'safety');
  return existsSync(dir) ? readdirSync(dir) : [];
}

describe('Atualizações dos mods (P1-12)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    // Sem verificação automática: o teste pede a verificação pelo botão.
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1, "updateCheckIntervalHours": 0 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('Verificar atualizações marca só os itens com versão nova (CA-T10-02)', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
    await modRow('Lithium').waitForDisplayed();
    // Antes de verificar, nada de faixa nem de botão Atualizar nas linhas.
    await expect($('button*=Revisar e atualizar')).not.toExist();
    await expect(modRow('Lithium').$('button*=Atualizar')).not.toExist();

    await $('button=Verificar atualizações').click();
    await $('.alert*=3 atualizações disponíveis.').waitForDisplayed({ timeout: 45_000 });
    for (const name of ['Lithium', 'Mod Menu', 'Fresh Animations']) {
      await expect(modRow(name).$('button=Atualizar')).toExist();
    }
    for (const name of ['Sodium', 'Jade', 'Text Placeholder API', 'Complementary']) {
      await expect(modRow(name).$('button=Atualizar')).not.toExist();
    }
    await expect(modRow('Sodium')).toHaveText(expect.stringContaining('Fixado'));
    await screenshot('30-atualizacoes-faixa');
    expect(await seriousViolations()).toEqual([]);
  });

  it('Atualizar um item: mostra agora e nova, as novidades, e grava só a referência (CA-T10-02)', async () => {
    const before = read('mods/lithium.pw.toml');
    await modRow('Lithium').$('button=Atualizar').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('Atualizar Lithium'));
    await browser.waitUntil(async () =>
      (await fullText('[role="dialog"]')).includes('mc1.21.1-0.15.5-fabric'),
    );
    const text = await fullText('[role="dialog"]');
    expect(text).toContain('mc1.21.1-0.15.4-fabric');
    expect(text).toContain('Corrige um travamento ao carregar chunks.');
    await screenshot('31-atualizar-um-item');
    expect(await seriousViolations()).toEqual([]);
    await dialog.$('button=Atualizar Lithium').click();
    await waitToast('Lithium foi atualizado para mc1.21.1-0.15.5-fabric.');
    const after = read('mods/lithium.pw.toml');
    expect(after).toContain('version = "E2ELITH01"');
    expect(after).toContain('filename = "lithium-fabric-0.15.5+mc1.21.1.jar"');
    expect(after).toContain(`name = "Lithium"`);
    expect(after).not.toContain('N08Z8wog');
    expect(before).toContain('N08Z8wog');
    // O índice foi atualizado: a mudança passou pelo packwiz.
    expect(read('index.toml')).toContain('mods/lithium.pw.toml');
    // Um item só: sem ponto de segurança.
    expect(safetyPoints()).toEqual([]);
    await $('.alert*=2 atualizações disponíveis.').waitForDisplayed({ timeout: 30_000 });
  });

  it('Revisar e atualizar: ponto de segurança antes; a faixa some depois', async () => {
    await $('button=Revisar e atualizar').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('Atualizar 2 itens'));
    await browser.waitUntil(async () => {
      const text = await fullText('[role="dialog"]');
      return text.includes('11.0.6') && text.includes('v1.10.5');
    });
    await expect(dialog).toHaveText(expect.stringContaining('ponto de segurança'));
    await screenshot('32-revisar-e-atualizar');
    expect(await seriousViolations()).toEqual([]);
    await dialog.$('button=Atualizar 2 itens').click();
    await waitToast('2 itens atualizados. Ponto de segurança criado antes.');
    expect(read('mods/modmenu.pw.toml')).toContain('version = "E2EMENU01"');
    expect(read('resourcepacks/fresh-animations.pw.toml')).toContain('version = "E2EFRESH01"');
    expect(safetyPoints()).toHaveLength(1);
    await browser.waitUntil(async () => !(await $('button=Revisar e atualizar').isExisting()), {
      timeout: 30_000,
    });
    await expect(modRow('Sodium')).toExist();
    await screenshot('33-pack-atualizado');
  });
});
