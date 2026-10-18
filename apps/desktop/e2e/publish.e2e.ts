/**
 * Publicar versão no GitHub para os jogadores no app real (V-03; SPEC T18), com o packwiz e o
 * git embutido de verdade e o GitHub simulado do servidor de fixtures (`mock-server/github.ts`:
 * API, servidor git com o `git receive-pack` do git instalado e `raw.githubusercontent.com`).
 *
 * - O token do GitHub é salvo em Configurações e o "Testar" confere na API (`GET /user`).
 * - Salvar a 1.0.0 como versão final e publicar pelo aviso de depois de salvar: primeira
 *   publicação cria o repositório público, o diálogo avisa que a versão não foi testada, e o
 *   resultado traz o link do pack (que serve o `pack.toml` publicado) e o passo a passo; a
 *   `main` do GitHub só tem o conteúdo do pack, `CHANGELOG.md` e `.gitattributes`, com a tag
 *   `v1.0.0` e a Release (CA-T18-04 ponta a ponta).
 * - O Histórico mostra o repositório, a versão publicada e o link.
 * - CA-T18-06: um config com um token `ghp_` bloqueia a publicação da 1.0.1, apontando arquivo
 *   e linha, e nada vai ao GitHub.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { GITHUB_LOGIN, GITHUB_TOKEN, type GithubState } from './mock-server/github';
import { replaceValue } from './fields';
import { dismissToasts } from './toasts';
import { openOrImport } from './open-or-import';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
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
const PACK = join(DATA_ROOT, 'fora', 'pack-publicar');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const REPO = `${GITHUB_LOGIN}/fixture-fabric`;
const PLAN_TITLE = (version: string) => `Publicar a versão ${version} para os jogadores`;

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function github(): Promise<GithubState> {
  const response = await fetch(`${MOCK_URL}/__github`);
  return (await response.json()) as GithubState;
}

/** Um diálogo pelo nome acessível (o título), quando ele aparece com o conteúdo pronto. */
async function dialogNamed(title: string, ready: string, timeout = 60_000) {
  const dialog = $(`[role="dialog"]*=${ready}`);
  await browser.waitUntil(
    async () =>
      (await dialog.isExisting()) &&
      (await dialog.getText()).includes(title) &&
      (await dialog.getText()).includes(ready),
    { timeout, timeoutMsg: `diálogo "${title}" com "${ready}" não apareceu` },
  );
  return dialog;
}

async function openSection(name: string, heading: string): Promise<void> {
  await dismissToasts();
  await $('nav[aria-label="Seções do pack"]').$(`a*=${name}`).click();
  await $(`h1=${heading}`).waitForDisplayed({ timeout: 30_000 });
}

async function saveFinal(version: string): Promise<void> {
  await dismissToasts();
  await $('header.packhead').$('button*=Salvar versão').click();
  const dialog = $('[role="dialog"]*=Número da versão');
  await dialog.waitForDisplayed({ timeout: 60_000 });
  const number = dialog.$('input.input');
  await replaceValue(number, version);
  await dialog.$('label*=Marcar como versão final').click();
  const save = dialog.$(`button*=Salvar versão ${version}`);
  await save.waitForEnabled({ timeout: 30_000 });
  await save.click();
  await $(`[role="dialog"]*=Versão ${version} salva como versão final`).waitForDisplayed({
    timeout: 60_000,
  });
}

describe('Publicar versão no GitHub (V-03; T18)', () => {
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
    rmSync(join(DATA_ROOT, 'cofre-de-teste', 'github-token'), { force: true });
  });

  it('salva o token do GitHub e o Testar confere na API', async () => {
    const link = $('header.topbar').$('a*=Configurações');
    await link.waitForClickable();
    await link.click();
    const row = $('[data-testid="chave-github"]');
    await row.waitForDisplayed({ timeout: 30_000 });
    await row.$('input').setValue(GITHUB_TOKEN);
    await row.$('button=Salvar').click();
    await expect(row).toHaveText(expect.stringContaining('Configurada'), { wait: 15_000 });
    await row.$('button=Testar').click();
    await expect(row).toHaveText(expect.stringContaining('Funcionando'), { wait: 30_000 });
    const html = await browser.execute(() => document.documentElement.outerHTML);
    expect(html.includes(GITHUB_TOKEN)).toBe(false);
  });

  it('primeira publicação: cria o repositório público, avisa que não foi testada e publica; axe', async () => {
    await dismissToasts();
    await $('header.topbar').$('a*=Meus packs').click();
    writeFileSync(PICK_FILE, PACK);
    await openOrImport();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });

    await saveFinal('1.0.0');
    await $('[role="dialog"]').$('button=Publicar versão 1.0.0').click();

    const setup = await dialogNamed('Primeira publicação deste pack', 'Quem pode ver');
    await expect(setup.$('input.input--mono')).toHaveValue('fixture-fabric');
    await expect(setup).toHaveText(expect.stringContaining('Nada do seu computador vai junto.'));
    await screenshot('70-primeira-publicacao');
    expect(await seriousViolationSummaries()).toEqual([]);
    await setup.$('button=Criar repositório e continuar').click();

    const plan = await dialogNamed(PLAN_TITLE('1.0.0'), 'A versão 1.0.0 não foi testada.', 120_000);
    await expect(plan).toHaveText(
      expect.stringContaining('Primeira publicação: tudo o que o pack tem.'),
    );
    await expect(plan).toHaveText(expect.stringContaining(`Vai para o GitHub (${REPO})`));
    await screenshot('71-publicar-avisos');
    expect(await seriousViolationSummaries()).toEqual([]);
    await plan.$('textarea').setValue('Façam backup dos mundos.');
    await plan.$('button=Publicar mesmo assim').click();

    const result = await dialogNamed(
      'Versão 1.0.0 publicada',
      'Como os jogadores instalam',
      120_000,
    );
    const packLink = `${MOCK_URL}/github/raw/${REPO}/main/pack.toml`;
    await expect(result.$('.linkbox input')).toHaveValue(packLink);
    await expect(result).toHaveText(
      expect.stringContaining(`"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${packLink}`),
    );
    await screenshot('72-publicado');
    expect(await seriousViolationSummaries()).toEqual([]);

    // O link do pack serve o pack.toml da versão, com os mesmos bytes.
    const served = await fetch(packLink);
    expect(served.status).toBe(200);
    expect(Buffer.from(await served.arrayBuffer())).toEqual(readFileSync(join(PACK, 'pack.toml')));
    const state = await github();
    expect(state.repos[REPO]?.private).toBe(false);
    expect(state.releases).toHaveLength(1);
    expect(state.releases[0]).toMatchObject({ tag: 'v1.0.0', name: 'Fixture Fabric 1.0.0' });
    expect(state.releases[0]?.body).toContain('Façam backup dos mundos.');
    const bare = state.repos[REPO]?.dir ?? '';
    const files = execFileSync('git', ['-C', bare, 'ls-tree', '-r', '--name-only', 'main'], {
      encoding: 'utf8',
    })
      .trim()
      .split('\n');
    expect(files).toContain('CHANGELOG.md');
    expect(files).toContain('.gitattributes');
    expect(files).toContain('pack.toml');
    expect(files).toContain('index.toml');
    expect(files.some((file) => file.startsWith('.warden'))).toBe(false);
    expect(
      execFileSync('git', ['-C', bare, 'log', '--format=%s', 'main'], { encoding: 'utf8' }).trim(),
    ).toBe('Fixture Fabric 1.0.0');
    expect(execFileSync('git', ['-C', bare, 'tag', '-l'], { encoding: 'utf8' }).trim()).toBe(
      'v1.0.0',
    );
    // O token nunca vai para o repositório do pack.
    expect(readFileSync(join(PACK, '.git', 'config'), 'utf8')).not.toContain(GITHUB_TOKEN);

    await result.$('button=Fechar').click();
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 15_000,
    });
  });

  it('o Histórico mostra o repositório, a versão publicada e o link do pack', async () => {
    await openSection('Histórico', 'Histórico');
    const panel = $('section[aria-labelledby="historico-publicacao"]');
    await expect(panel).toHaveText(expect.stringContaining(REPO), { wait: 30_000 });
    await expect(panel).toHaveText(expect.stringContaining('Público'));
    await expect(panel).toHaveText(expect.stringContaining('1.0.0, em'));
    await expect(panel.$('#historico-link-do-pack')).toHaveValue(
      `${MOCK_URL}/github/raw/${REPO}/main/pack.toml`,
    );
    const item = $('#versao-1-0-0');
    await expect(item).toHaveText(expect.stringContaining('Publicada'));
    await expect(item.$('button*=Publicar versão')).not.toBeExisting();
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('CA-T18-06: um token ghp_ num config bloqueia a publicação apontando arquivo e linha', async () => {
    writeFileSync(
      join(PACK, 'config', 'segredo.toml'),
      '[bot]\nchave = "ghp_abcdefghijklmnopqrstuvwxyz0123456789"\n',
    );
    await openSection('Mods', 'Mods');
    await browser.execute(() => {
      document.dispatchEvent(new Event('visibilitychange', { bubbles: true }));
    });
    await expect($('header.packhead')).toHaveText(expect.stringMatching(/1\s+alteração/), {
      wait: 30_000,
    });
    await saveFinal('1.0.1');
    await $('[role="dialog"]').$('button=Publicar versão 1.0.1').click();
    const plan = await dialogNamed(
      PLAN_TITLE('1.0.1'),
      'Encontramos algo que parece uma chave em config/segredo.toml, linha 2.',
      120_000,
    );
    await expect(plan.$('button=Publicar mesmo assim')).toBeDisabled();
    await expect(plan).toHaveText(
      expect.stringContaining('Mudanças desde a 1.0.0, a última publicada.'),
    );
    await screenshot('73-publicar-bloqueado');
    expect(await seriousViolationSummaries()).toEqual([]);
    await plan.$('button=Cancelar').click();
    const state = await github();
    expect(state.releases).toHaveLength(1);
    expect(
      execFileSync('git', ['-C', state.repos[REPO]?.dir ?? '', 'tag', '-l'], {
        encoding: 'utf8',
      }).trim(),
    ).toBe('v1.0.0');
  });
});
