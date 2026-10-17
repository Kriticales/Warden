/**
 * "Testar como o jogador recebe" no app real (L-07; SPEC T13; CA-T13-14), com o jogo simulado
 * da `warden-launcher` no lugar do Minecraft, o packwiz-installer-bootstrap e o
 * packwiz-installer **reais** (do cache da `cargo xtask installer`, servidos pelo servidor de
 * fixtures como a URL de download dos Releases) e o GitHub simulado (`mock-server/github.ts`).
 *
 * - Pack ainda não publicado: o Warden avisa e instala do zero o pack exportado agora, servido
 *   só neste computador, numa instância temporária limpa (com o Crash Assistant e sem os
 *   mundos), que é apagada no fim; o resultado diz o que o jogador recebeu de diferente.
 * - Depois de publicar a 1.0.0: instala pelo link do pack (o `raw.githubusercontent.com`
 *   simulado), sem nenhuma chamada à API do GitHub.
 *
 * Sem Java ou sem os jars do instalador, avisa e pula. Com `WARDEN_E2E_SCREENSHOTS=<pasta>`,
 * grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { GITHUB_LOGIN, GITHUB_TOKEN } from './mock-server/github';
import { jogadorJar } from './mock-server/player';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const READY =
  process.env.WARDEN_E2E_HAS_JAVA === '1' && process.env.WARDEN_E2E_HAS_INSTALLER === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const PACK = join(DATA_ROOT, 'fora', 'pack-jogador');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const TMP = join(DATA_ROOT, 'data', 'cache', 'tmp');
const REPO = `${GITHUB_LOGIN}/jogador-de-teste`;
const INSTANCES = join(DATA_ROOT, 'data', 'instances');

/** As instâncias de teste que já existiam antes deste spec (de outros specs). */
let instancesBefore = new Set<string>();

function instanceIds(): string[] {
  return existsSync(INSTANCES) ? readdirSync(INSTANCES) : [];
}

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Metafile de um mod servido em `/jogador/` pelo servidor de fixtures. */
function metafile(name: string, file: string, side: string, extra = ''): string {
  return [
    `name = "${name}"`,
    `filename = "${file}"`,
    `side = "${side}"`,
    '',
    '[download]',
    `url = "${MOCK_URL}/jogador/${file}"`,
    'hash-format = "sha256"',
    `hash = "${sha256(jogadorJar(file))}"`,
    extra,
  ].join('\n');
}

/**
 * Um pack vanilla 1.20.1 com uma config, o Crash Assistant (marcado como ferramenta do
 * jogador) e um opcional que vem desmarcado. Sem `.warden-fake-game`, o jogo simulado fica
 * aberto (com um processo filho) até "Parar jogo".
 */
function writePack(): void {
  const files: Record<string, string> = {
    'config/jogador.toml': 'dificuldade = 2\n',
    'mods/crash-assistant.pw.toml': metafile('Crash Assistant', 'crash-assistant.jar', 'client'),
    'mods/zoom.pw.toml': metafile(
      'Zoom',
      'zoom.jar',
      'client',
      '\n[option]\noptional = true\ndefault = false\n',
    ),
  };
  mkdirSync(PACK, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, text]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, text);
      const meta = path.endsWith('.pw.toml') ? 'metafile = true\n' : '';
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(text)}"\n${meta}`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Jogador de Teste"',
      'author = "Warden"',
      'version = "0.1.0"',
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      `hash = "${sha256(index)}"`,
      '',
      '[versions]',
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
  mkdirSync(join(PACK, '.warden'), { recursive: true });
  writeFileSync(
    join(PACK, '.warden', 'project.toml'),
    '[player-tools]\ncrash-assistant = "mods/crash-assistant.pw.toml"\n',
  );
}

/** A instância limpa do teste em andamento (`cache/tmp/player-test-*`), se houver. */
function cleanGameDir(): string | null {
  if (!existsSync(TMP)) return null;
  const found = readdirSync(TMP).find((name) => name.startsWith('player-test-'));
  return found ? join(TMP, found, 'minecraft') : null;
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 120_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** "Testar como o jogador recebe" pelo menu ▾ do Testar, até o jogo simulado abrir. */
async function testAsPlayer(): Promise<void> {
  await dismissToasts();
  await $('header.packhead .testbtn__more').click();
  const item = $('[role="menuitem"]*=Testar como o jogador recebe');
  await item.waitForDisplayed();
  await expect(item).toHaveText(
    expect.stringContaining('Instala pelo link do pack, como um jogador'),
  );
  await item.click();
  await waitHeading('Jogo aberto');
  const console = $('section[aria-label="Console do jogo"]');
  await browser.waitUntil(async () => (await console.getText()).includes('pai vivo'), {
    timeout: 30_000,
    timeoutMsg: 'o console não mostrou o jogo simulado',
  });
}

/** "Parar jogo" e o resultado "Como o jogador recebe". */
async function stopAndReadResult() {
  await dismissToasts();
  await $('main').$('button=Parar jogo').click();
  const dialog = $('[role="alertdialog"]');
  await dialog.waitForDisplayed();
  await dialog.$('button=Parar jogo').click();
  await waitHeading('Encerrado por você', 30_000);
  const result = $('section[aria-labelledby="player-result-title"]');
  await result.waitForDisplayed({ timeout: 30_000 });
  return result;
}

async function requests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

describe('Testar como o jogador recebe (L-07; T13)', function () {
  before(async function () {
    if (!READY) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java ou sem os jars do instalador (cargo xtask installer): E2E pulado');
      this.skip();
    }
    instancesBefore = new Set(instanceIds());
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(async () => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'cofre-de-teste', 'github-token'), { force: true });
    // O que este spec criou e outros specs não esperam: a pasta de sessões do pack e o
    // repositório e a Release no GitHub simulado.
    for (const id of instanceIds()) {
      if (!instancesBefore.has(id)) {
        rmSync(join(INSTANCES, id), { recursive: true, force: true });
      }
    }
    await fetch(`${MOCK_URL}/__github/reset`, { method: 'POST' });
  });

  it('pack não publicado: avisa e instala do zero o pack exportado agora, numa instância limpa; axe', async () => {
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    const open = $('button=Abrir pack');
    const notNow = $('button=Agora não');
    await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
      timeout: 60_000,
      timeoutMsg: 'Abrir pack não ficou pronto',
    });
    if (await notNow.isExisting()) await notNow.click();
    else await open.click();
    await waitHeading('Mods', 60_000);

    await testAsPlayer();
    await expect($('main')).toHaveText(
      expect.stringContaining('O pack ainda não foi publicado para os jogadores'),
    );
    await screenshot('90-jogador-jogo-aberto');
    // A instância temporária: o pack do zero, com o Crash Assistant e sem mundos.
    const game = cleanGameDir();
    expect(game).not.toBeNull();
    expect(existsSync(join(game ?? '', 'mods', 'crash-assistant.jar'))).toBe(true);
    expect(existsSync(join(game ?? '', 'config', 'jogador.toml'))).toBe(true);
    expect(existsSync(join(game ?? '', 'saves'))).toBe(false);

    const result = await stopAndReadResult();
    await expect(result).toHaveText(
      expect.stringContaining(
        'O pack ainda não foi publicado: instalado do zero pelo pack exportado agora, servido só neste computador.',
      ),
    );
    await expect(result).toHaveText(expect.stringContaining('O jogador recebeu 3 arquivos.'));
    await expect(result).toHaveText(
      expect.stringContaining('O Crash Assistant (ferramenta do jogador) chegou ao jogador.'),
    );
    await expect(result.$('table')).toHaveText(
      expect.stringContaining('Opcional: o jogador escolhe na instalação (vem desmarcado).'),
    );
    await screenshot('91-jogador-resultado');
    expect(await seriousViolationSummaries()).toEqual([]);
    expect(cleanGameDir()).toBeNull();
  });

  it('salva o token do GitHub e volta ao pack pela linha de Meus packs, já com o último teste', async () => {
    // Token do GitHub em Configurações (pela tela Meus packs).
    await dismissToasts();
    await $('header.packhead').$('a*=Meus packs').click();
    const settings = $('header.topbar').$('a*=Configurações');
    await settings.waitForClickable({ timeout: 30_000 });
    await settings.click();
    const row = $('[data-testid="chave-github"]');
    await row.waitForDisplayed({ timeout: 30_000 });
    await row.$('input').setValue(GITHUB_TOKEN);
    await row.$('button=Salvar').click();
    await expect(row).toHaveText(expect.stringContaining('Configurada'), { wait: 15_000 });

    // Volta ao pack e publica a 1.0.0.
    await dismissToasts();
    await $('header.topbar').$('a*=Meus packs').click();
    // O "último teste" do pack já mostra o teste como o jogador (dados atuais).
    const packRow = $('tr.packrow*=Jogador de Teste');
    await packRow.waitForDisplayed({ timeout: 30_000 });
    await expect(packRow).not.toHaveText(expect.stringContaining('Nunca testado'));
    await packRow.$('button*=Jogador de Teste').click();
    await waitHeading('Mods', 60_000);
  });

  it('salva a 1.0.0 como versão final e publica', async () => {
    await dismissToasts();
    await $('header.packhead').$('button*=Salvar versão').click();
    const save = $('[role="dialog"]*=Número da versão');
    await save.waitForDisplayed({ timeout: 60_000 });
    await save.$('input.input').setValue('1.0.0');
    await save.$('label*=Marcar como versão final').click();
    const saveButton = save.$('button*=Salvar versão 1.0.0');
    await saveButton.waitForEnabled({ timeout: 30_000 });
    await saveButton.click();
    await $('[role="dialog"]*=Versão 1.0.0 salva como versão final').waitForDisplayed({
      timeout: 60_000,
    });
    await $('[role="dialog"]').$('button=Publicar versão 1.0.0').click();
    const setup = $('[role="dialog"]*=Quem pode ver');
    await setup.waitForDisplayed({ timeout: 60_000 });
    await setup.$('button=Criar repositório e continuar').click();
    const plan = $('[role="dialog"]*=Publicar a versão 1.0.0 para os jogadores');
    const publish = plan.$('button=Publicar mesmo assim');
    await publish.waitForEnabled({ timeout: 120_000 });
    await publish.click();
    const done = $('[role="dialog"]*=Como os jogadores instalam');
    await done.waitForDisplayed({ timeout: 120_000 });
    const packLink = `${MOCK_URL}/github/raw/${REPO}/main/pack.toml`;
    await expect(done.$('.linkbox input')).toHaveValue(packLink);
    await done.$('button=Fechar').click();
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 15_000,
      timeoutMsg: 'o diálogo da publicação não fechou',
    });
  });

  it('CA-T13-14: depois de publicar, instala pelo link do pack sem chamar a API do GitHub', async () => {
    // Daqui em diante, só o que o teste como o jogador pede.
    await fetch(`${MOCK_URL}/__reset`);
    await testAsPlayer();
    expect(cleanGameDir()).not.toBeNull();
    const result = await stopAndReadResult();
    await expect(result).toHaveText(
      expect.stringContaining('Instalado do zero pelo link do pack (versão 1.0.0 publicada).'),
    );
    await expect(result).not.toHaveText(expect.stringContaining('versão anterior'));
    await screenshot('92-jogador-pelo-link');
    expect(await seriousViolationSummaries()).toEqual([]);

    const seen = await requests();
    expect(seen).toContain(`GET /github/raw/${REPO}/main/pack.toml`);
    expect(seen).toContain(`GET /github/raw/${REPO}/main/index.toml`);
    expect(seen.filter((line) => line.includes('/github/api/'))).toEqual([]);
    expect(cleanGameDir()).toBeNull();
  });
});
