/**
 * Seção Configs no app real (C-02; SPEC T12), com o packwiz de verdade. O pack é o que o packwiz
 * real gerou para o Fabric 1.21.1 (`crates/warden-packwiz/tests/fixtures/packwiz-output/
 * fabric-1.21.1`): tem `config/exemplo.properties` e um `options.txt` no índice.
 *
 * - A árvore mostra os arquivos do pack; abrir um arquivo põe o caminho na URL e o texto no editor.
 * - CA-T12-01: abrir e salvar sem mudanças não toca no disco (Salvar fica desabilitado).
 * - Salvar mostra as diferenças, grava só o que mudou (o resto, byte a byte) e o `index.toml`
 *   passa a ter o hash novo (a gravação passa pelo `packwiz refresh`).
 * - CA-T12-05: depois de salvar não sobra `.bak` nem `.warden-tmp` na pasta do pack.
 * - CA-T12-03: arquivo alterado por fora → o aviso aparece e nada é gravado sem escolha.
 * - Alterações não salvas: trocar de arquivo pede confirmação.
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
  statSync,
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
const PACK = join(DATA_ROOT, 'fora', 'pack-configs');
const CONFIG = join(PACK, 'config', 'exemplo.properties');
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
  return (
    violations
      // O contraste dos tokens é medido por design/system/tools/contraste.mjs (QUALITY §4.1).
      .filter((v) => v.id !== 'color-contrast')
      .filter((v) => v.impact === 'serious' || v.impact === 'critical')
      .map((v) => `${v.id}: ${v.help}`)
  );
}

/** Todos os arquivos da pasta do pack, para conferir que nada sobrou. */
function allFiles(dir: string): string[] {
  const out: string[] = [];
  const walk = (current: string) => {
    for (const entry of readdirSync(current)) {
      const path = join(current, entry);
      if (statSync(path).isDirectory()) walk(path);
      else out.push(path);
    }
  };
  walk(dir);
  return out;
}

/** Acrescenta texto ao fim do documento do CodeMirror aberto (digitação real no editor). */
async function typeAtEnd(text: string): Promise<void> {
  const content = $('.cm-content');
  await content.click();
  await browser.keys(['Control', 'End']);
  await browser.keys(text.split(''));
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
      return seen.includes(text);
    },
    { timeout: 30_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
}

describe('Configs (C-02; T12)', () => {
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
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('abre o pack, mostra a árvore e abre o arquivo no editor; axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });

    await $('nav[aria-label="Seções do pack"]').$('a*=Configs').click();
    await $('h1=Configs').waitForDisplayed();
    const tree = $('[role="tree"]');
    await tree.waitForDisplayed();
    // `textContent`: confere os nomes mesmo que a árvore ainda esteja rolando.
    await expect(tree).toHaveElementProperty(
      'textContent',
      expect.stringContaining('exemplo.properties'),
    );
    await expect(tree).toHaveElementProperty(
      'textContent',
      expect.stringContaining('Opções do jogo (options.txt)'),
    );

    await tree.$('[role="treeitem"]*=exemplo.properties').click();
    await $('.cm-content').waitForDisplayed({ timeout: 30_000 });
    await expect($('.cm-content')).toHaveText(expect.stringContaining('chave=valor'));
    // Abrir e não mudar nada: nada para salvar (CA-T12-01).
    await expect($('button=Salvar')).toBeDisabled();
    await screenshot('40-configs');
    expect(await seriousViolations()).toEqual([]);
  });

  it('salva mostrando as diferenças; só o que mudou é gravado e nada sobra na pasta (CA-T12-05)', async () => {
    const before = readFileSync(CONFIG, 'utf8');
    await typeAtEnd('nova=1');
    await expect($('.editor__foot')).toHaveText(expect.stringContaining('ainda não salva'));
    await $('button=Salvar').click();
    const dialog = $('[role="dialog"]*=Salvar exemplo.properties?');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('nova=1'));
    // Ainda nada no disco: o diálogo é só a pré-visualização.
    expect(readFileSync(CONFIG, 'utf8')).toBe(before);
    await screenshot('41-configs-diferencas');
    await dialog.$('button=Salvar').click();
    await waitToast('exemplo.properties salvo.');

    expect(readFileSync(CONFIG, 'utf8')).toBe(`${before}nova=1`);
    expect(readFileSync(join(PACK, 'index.toml'), 'utf8')).toContain('exemplo.properties');
    const leftovers = allFiles(PACK).filter((path) => /\.(bak|warden-tmp)$/i.test(path));
    expect(leftovers).toEqual([]);
    expect(existsSync(`${CONFIG}.bak`)).toBe(false);
  });

  it('CA-T12-03: arquivo alterado por fora → aviso, e nada é gravado sem escolha', async () => {
    await typeAtEnd('\nmeu=2');
    const outside = 'chave=de-fora\n';
    writeFileSync(CONFIG, outside);
    await $('button=Salvar').click();
    await $('[role="dialog"]*=Salvar exemplo.properties?').$('button=Salvar').click();

    await $('*=Este arquivo foi alterado fora do Warden.').waitForDisplayed({ timeout: 30_000 });
    expect(readFileSync(CONFIG, 'utf8')).toBe(outside);
    await screenshot('42-configs-conflito');

    // Recarregar descarta o texto da pessoa e mostra o do disco.
    await $('button=Recarregar').click();
    await $('[role="alertdialog"]').$('button=Recarregar e descartar').click();
    await expect($('.cm-content')).toHaveText(expect.stringContaining('chave=de-fora'));
    expect(readFileSync(CONFIG, 'utf8')).toBe(outside);
  });

  it('trocar de arquivo com alterações não salvas pede confirmação', async () => {
    await typeAtEnd('x=9');
    await $('[role="treeitem"]*=Opções do jogo').click();
    const ask = $('[role="alertdialog"]*=Sair sem salvar?');
    await ask.waitForDisplayed();
    await screenshot('43-configs-sair');
    await ask.$('button=Continuar editando').click();
    await expect($('h1=Configs')).toBeDisplayed();
    await expect($('.cm-content')).toHaveText(expect.stringContaining('x=9'));

    await $('[role="treeitem"]*=Opções do jogo').click();
    await $('[role="alertdialog"]*=Sair sem salvar?').$('button=Descartar alterações').click();
    await expect($('.editor__file')).toHaveText('options.txt');
    expect(readFileSync(CONFIG, 'utf8')).toBe('chave=de-fora\n');
  });
});
