/**
 * Salvar versão e Histórico no app real (V-02; SPEC T16 e T17), com o packwiz e o git de
 * verdade. O pack é o que o packwiz real gerou para o Fabric 1.21.1 (`crates/warden-packwiz/
 * tests/fixtures/packwiz-output/fabric-1.21.1`, versão 1.0.0 no `pack.toml`).
 *
 * - CA-T16-01/03: o diálogo sugere a versão, o changelog sai com as notas, o `pack.toml` e o
 *   `CHANGELOG.md` ficam como devem e `git status` fica limpo (conferido com o git do
 *   computador, que o app não usa).
 * - CA-T16-04: 0.9.0 depois de 1.0.0 é recusado com a explicação, ainda no diálogo.
 * - CA-T16-05: "Marcar como versão final" cria só a referência de versão final; marcar e
 *   desmarcar depois não muda nenhum arquivo do pack; nada é publicado (o repositório não tem
 *   remoto).
 * - CA-T17-01/03: voltar para a 1.0.0 deixa a pasta com os mesmos bytes que ela tinha ao salvar
 *   (arquivos criados depois somem) e o ponto de segurança recupera o estado de antes.
 * - Descartar a alteração de uma config pede confirmação e apaga o arquivo novo.
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
const PACK = join(DATA_ROOT, 'fora', 'pack-versoes');
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

/** Os arquivos do pack (caminhos com `/`), sem a pasta `.git`, com o resumo do conteúdo. */
function treeHashes(dir: string = PACK): Record<string, string> {
  const out: Record<string, string> = {};
  const walk = (current: string) => {
    for (const entry of readdirSync(current)) {
      if (entry === '.git') continue;
      const path = join(current, entry);
      if (statSync(path).isDirectory()) walk(path);
      else {
        out[relative(dir, path).replaceAll('\\', '/')] = createHash('sha256')
          .update(readFileSync(path))
          .digest('hex');
      }
    }
  };
  walk(dir);
  return out;
}

function git(...args: string[]): string {
  return execFileSync('git', ['-C', PACK, ...args], { encoding: 'utf8' }).trim();
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
      return seen.includes(text) || seen.includes('Não foi possível');
    },
    { timeout: 60_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
  expect(seen).toContain(text);
}

/**
 * A pessoa volta para a janela depois de mexer nos arquivos por fora: o TanStack Query relê o
 * que está na tela quando a janela volta ao foco (o WebDriver não dispara esse evento sozinho).
 */
async function refocus(): Promise<void> {
  await browser.execute(() => {
    document.dispatchEvent(new Event('visibilitychange', { bubbles: true }));
  });
}

/** Fecha os avisos (toasts): eles ficam no canto de baixo e cobririam os botões da página. */
async function dismissToasts(): Promise<void> {
  await browser.execute(() => {
    for (const close of document.querySelectorAll<HTMLButtonElement>('.toast button:last-child')) {
      close.click();
    }
  });
  await browser.pause(300);
}

async function openSection(name: string, heading: string): Promise<void> {
  await dismissToasts();
  await $('nav[aria-label="Seções do pack"]').$(`a*=${name}`).click();
  await $(`h1=${heading}`).waitForDisplayed({ timeout: 30_000 });
}

/** Abre o diálogo Salvar versão pelo botão do cabeçalho e espera o formulário. */
async function openSaveDialog(): Promise<ReturnType<typeof $>> {
  await $('header.packhead').$('button*=Salvar versão').click();
  const dialog = $('[role="dialog"]*=Número da versão');
  await dialog.waitForDisplayed({ timeout: 60_000 });
  return $('[role="dialog"]');
}

/**
 * Espera o aviso "Versão X salva…" depois de clicar em Salvar. Se o diálogo mostrar um erro, o
 * teste falha na hora com o texto dele (e não com um tempo esgotado sem explicação).
 */
async function waitSaved(text: string): Promise<void> {
  let failure = '';
  await browser
    .waitUntil(
      async () => {
        if (await $(`[role="dialog"]*=${text}`).isExisting()) return true;
        const panel = $('[role="dialog"] .alert--danger');
        if (await panel.isExisting()) failure = await panel.getText();
        return failure !== '';
      },
      { timeout: 45_000, timeoutMsg: `o aviso "${text}" não apareceu` },
    )
    .catch(async (error: unknown) => {
      await screenshot('99-falha-ao-salvar');
      throw error;
    });
  expect(failure).toBe('');
}

async function closeDialog(): Promise<void> {
  await $('[role="dialog"]').$('button=Fechar').click();
  await $('[role="dialog"]').waitForExist({ reverse: true });
}

describe('Salvar versão e Histórico (V-02; T16 e T17)', () => {
  let at100: Record<string, string> = {};
  let beforeRestore: Record<string, string> = {};

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
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('abre o pack e o Histórico mostra tudo como não salvo, sem versões; axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    pickNext(PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });

    await openSection('Histórico', 'Histórico');
    await $('h2=Versões').waitForDisplayed({ timeout: 30_000 });
    const main = $('main');
    await expect(main).toHaveText(expect.stringContaining('Nenhuma versão salva ainda'));
    await expect(main).toHaveText(expect.stringContaining('Não salvas'));
    await expect(main).toHaveText(expect.stringContaining('desde a criação do pack'));
    await expect(main).toHaveText(expect.stringContaining('Publicação para os jogadores'));
    await screenshot('40-historico-vazio');
    expect(await seriousViolations()).toEqual([]);
  });

  it('CA-T16-01 e CA-T16-03: sugere a versão do pack, salva como versão final e deixa o git limpo; axe', async () => {
    const dialog = await openSaveDialog();
    const number = dialog.$('input.input');
    // Primeira versão: usa a versão que o pack já tem.
    await expect(number).toHaveValue('1.0.0');
    await expect(dialog).toHaveText(expect.stringContaining('é a primeira versão salva'));
    await expect(dialog).toHaveText(expect.stringContaining('### Mods adicionados'));
    await dialog.$('textarea').setValue('Primeira versão do Vale');
    await dialog.$('label*=Marcar como versão final').click();
    await screenshot('41-salvar-versao');
    expect(await seriousViolations()).toEqual([]);

    const save = dialog.$('button*=Salvar versão 1.0.0');
    await save.waitForEnabled({ timeout: 15_000 });
    await save.click();
    const notice = $('[role="dialog"]*=salva como versão final');
    await waitSaved('salva como versão final');
    await expect(notice).toHaveText(
      expect.stringContaining('Os jogadores só recebem quando você publicar.'),
    );
    await screenshot('42-versao-salva');
    await closeDialog();

    // No disco: a versão no pack.toml, o changelog com as notas e o git sem pendências.
    expect(readFileSync(join(PACK, 'pack.toml'), 'utf8')).toContain('version = "1.0.0"');
    const changelog = readFileSync(join(PACK, 'CHANGELOG.md'), 'utf8');
    expect(changelog).toMatch(/^# Changelog\r?\n\r?\n## 1\.0\.0 — \d{4}-\d{2}-\d{2}/);
    expect(changelog).toContain('Primeira versão do Vale');
    expect(changelog).toContain('### Mods adicionados');
    expect(git('status', '--porcelain')).toBe('');
    expect(git('tag', '-l')).toBe('v1.0.0');
    expect(git('for-each-ref', '--format=%(refname)', 'refs/warden/final')).toBe(
      'refs/warden/final/v1.0.0',
    );
    // Salvar é local: o repositório não tem remoto.
    expect(git('remote')).toBe('');
    at100 = treeHashes();

    // O Histórico mostra a versão como final, não publicada, e o pack sem alterações.
    await openSection('Histórico', 'Histórico');
    const item = $('#versao-1-0-0');
    await item.waitForDisplayed({ timeout: 30_000 });
    await expect(item).toHaveText(expect.stringContaining('Versão final · não publicada'));
    await expect(item).toHaveText(expect.stringContaining('Primeira versão do Vale'));
    await expect(item.$('button*=Publicar versão 1.0.0')).toHaveAttribute('aria-disabled', 'true');
    await expect($('main')).toHaveText(expect.stringContaining('Nada mudou desde a versão 1.0.0.'));
    await expect($('header.packhead')).not.toHaveText(expect.stringMatching(/\d+\s+alterações/));
    await screenshot('43-historico');
    expect(await seriousViolations()).toEqual([]);
  });

  it('CA-T16-04: 0.9.0 depois da 1.0.0 é recusado com a explicação e 1.0.1 salva (só configs: correção)', async () => {
    await openSection('Mods', 'Mods');
    writeFileSync(join(PACK, 'config', 'extra.toml'), 'extra = true\n');
    await refocus();
    await expect($('header.packhead')).toHaveText(expect.stringMatching(/1\s+alteração/), {
      wait: 15_000,
    });
    const dialog = await openSaveDialog();
    const number = dialog.$('input.input');
    // Só uma config mudou: correção.
    await expect(number).toHaveValue('1.0.1');
    await expect(dialog).toHaveText(expect.stringContaining('porque você mudou 1 config'));
    await expect(dialog).toHaveText(expect.stringContaining('config/extra.toml'));

    await number.setValue('0.9.0');
    await expect(dialog).toHaveText(
      expect.stringContaining('não é maior que a última versão salva, 1.0.0'),
      { wait: 15_000 },
    );
    await expect(dialog.$('button*=Salvar versão 0.9.0')).toBeDisabled();
    await screenshot('44-versao-recusada');

    await number.setValue('1.0.1');
    const save = dialog.$('button*=Salvar versão 1.0.1');
    await save.waitForEnabled({ timeout: 15_000 });
    await save.click();
    await waitSaved('Versão 1.0.1 salva');
    await expect($('[role="dialog"]')).toHaveText(
      expect.stringContaining('Nada foi enviado ao GitHub.'),
    );
    await closeDialog();
    expect(git('status', '--porcelain')).toBe('');
    expect(git('tag', '-l')).toBe('v1.0.0\nv1.0.1');
    expect(readFileSync(join(PACK, 'CHANGELOG.md'), 'utf8')).toMatch(/## 1\.0\.1[\s\S]*## 1\.0\.0/);
  });

  it('CA-T16-05: marcar e desmarcar a versão final não muda nenhum arquivo do pack', async () => {
    await openSection('Histórico', 'Histórico');
    const before = treeHashes();
    const item = $('#versao-1-0-1');
    await item.waitForDisplayed({ timeout: 30_000 });
    await expect(item).toHaveText(expect.stringContaining('Só salva'));
    await item.$('button=Marcar como versão final').click();
    await waitToast('A versão 1.0.1 agora é uma versão final.');
    await expect(item).toHaveText(expect.stringContaining('Versão final · não publicada'));
    expect(git('for-each-ref', '--format=%(refname)', 'refs/warden/final')).toContain(
      'refs/warden/final/v1.0.1',
    );
    expect(treeHashes()).toEqual(before);

    await item.$('button=Desmarcar versão final').click();
    await waitToast('A versão 1.0.1 deixou de ser versão final');
    await expect(item).toHaveText(expect.stringContaining('Só salva'));
    expect(git('for-each-ref', '--format=%(refname)', 'refs/warden/final')).not.toContain('v1.0.1');
    expect(treeHashes()).toEqual(before);
  });

  it('CA-T17-01 e CA-T17-03: voltar para a 1.0.0 iguala a pasta e o ponto de segurança recupera o estado de antes', async () => {
    // Alteração não salva por cima da 1.0.1.
    writeFileSync(join(PACK, 'config', 'depois.toml'), 'depois = 1\n');
    await openSection('Mods', 'Mods');
    await openSection('Histórico', 'Histórico');
    await expect($('main')).toHaveText(expect.stringContaining('config/depois.toml'));
    beforeRestore = treeHashes();
    expect(beforeRestore).not.toEqual(at100);

    await dismissToasts();
    await $('#versao-1-0-0').$('button=Voltar para esta versão').click();
    const confirm = $('[role="alertdialog"]');
    await confirm.waitForDisplayed();
    await expect(confirm).toHaveText(
      expect.stringContaining('O pack fica exatamente como estava na 1.0.0.'),
    );
    await expect(confirm).toHaveText(expect.stringContaining('ponto de segurança'));
    await screenshot('45-voltar');
    await confirm.$('button=Voltar para 1.0.0').click();
    await waitToast('O pack voltou para a versão 1.0.0');

    // A pasta tem os mesmos bytes que tinha ao salvar a 1.0.0; os arquivos de depois sumiram.
    expect(treeHashes()).toEqual(at100);
    expect(existsSync(join(PACK, 'config', 'extra.toml'))).toBe(false);
    expect(existsSync(join(PACK, 'config', 'depois.toml'))).toBe(false);
    expect(git('tag', '-l')).toBe('v1.0.0\nv1.0.1');

    // O estado de antes está no ponto de segurança.
    await dismissToasts();
    await $('button=Pontos de segurança…').click();
    const points = $('[role="dialog"]*=Pontos de segurança');
    await points.waitForDisplayed();
    await expect(points).toHaveText(expect.stringContaining('antes de voltar para 1.0.0'));
    await screenshot('46-pontos-de-seguranca');
    // O nome acessível (com a data) está no aria-label; o texto visível é só "Recuperar".
    await points.$('.//button[starts-with(@aria-label, "Recuperar o ponto de segurança")]').click();
    const recover = $('[role="alertdialog"]');
    await recover.waitForDisplayed();
    await recover.$('button=Recuperar ponto').click();
    await waitToast('Ponto de segurança recuperado');
    expect(treeHashes()).toEqual(beforeRestore);
    await points.$('button=Fechar').click();
  });

  it('descartar a alteração de uma config pede confirmação e apaga o arquivo novo', async () => {
    // O estado recuperado tem `config/depois.toml` como alteração não salva.
    await openSection('Mods', 'Mods');
    await openSection('Histórico', 'Histórico');
    // O nome acessível está no aria-label; o texto visível do botão é só "Descartar".
    const button = $('//button[@aria-label="Descartar a alteração de config/depois.toml"]');
    await button.waitForDisplayed({ timeout: 30_000 });
    await button.click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(
      expect.stringContaining('Descartar a alteração de config/depois.toml?'),
    );
    await screenshot('47-descartar');
    await dialog.$('button=Descartar alteração').click();
    await waitToast('Alteração descartada');
    expect(existsSync(join(PACK, 'config', 'depois.toml'))).toBe(false);
    expect(git('status', '--porcelain')).toBe('');
  });
});
