/**
 * Testar como servidor no app real (L-09; SPEC T27 e T21), com o jogo simulado da
 * `warden-launcher` (modo `conectar`, que entra no servidor pelo Quick Play) e o servidor
 * simulado da `warden-server` (`WardenFakeServer`: lê o `eula.txt` e o `server.properties`,
 * escuta na porta, escreve o "Done", aceita comandos e o `stop`). O app não baixa Java, jogo nem
 * servidor; todo o resto é o de verdade. O modo do servidor simulado vem do arquivo
 * `.warden-fake-server` na pasta do pack.
 *
 * - CA-T27-02 / CA-T21-05: sem a EULA, "Agora não" não abre nada nem grava `eula=true`; o
 *   aceite fica gravado (Configurações mostra a data, e Revogar volta a "Ainda não aceita").
 * - CA-T27-03: com a memória livre (simulada) menor que servidor + jogo, o aviso aparece antes
 *   de abrir.
 * - CA-T27-01 (parte do app): o servidor abre com o `server.properties` controlado, o jogo
 *   entra nele, o console do servidor é separado do do jogo e recebe comandos; "Parar jogo e
 *   servidor" fecha os dois e a porta fica livre (CA-T27-05).
 * - CA-T27-04: um mod de cliente que derruba o servidor termina em "O servidor travou", com o
 *   mod e o trecho do log; o "Só cliente" não vai para a pasta do servidor.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { dismissToasts } from './toasts';

const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const FREE_MEMORY_FILE = env('WARDEN_E2E_FREE_MEMORY_FILE');
const PACK = join(DATA_ROOT, 'fora', 'pack-servidor');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

/** Um metafile "Só cliente" com download que não existe: nunca é baixado para o servidor. */
const CLIENT_ONLY = [
  'name = "Zoom de Cliente"',
  'filename = "so-cliente.jar"',
  'side = "client"',
  '',
  '[download]',
  'url = "https://127.0.0.1:9/so-cliente.jar"',
  'hash-format = "sha256"',
  `hash = "${'0'.repeat(64)}"`,
  '',
].join('\n');

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 com os arquivos dados no índice e os modos do jogo e do servidor. */
function writePack(files: Record<string, string>, fakeGame: string, fakeServer: string): void {
  rmSync(PACK, { recursive: true, force: true });
  mkdirSync(PACK, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, text]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, text);
      const metafile = path.endsWith('.pw.toml') ? 'metafile = true\n' : '';
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(text)}"\n${metafile}`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale do Servidor"',
      'author = "Warden"',
      'version = "1.0.0"',
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
  writeFileSync(join(PACK, '.warden-fake-game'), fakeGame);
  writeFileSync(join(PACK, '.warden-fake-server'), fakeServer);
}

/** A pasta do servidor local do pack (`instances/<pack>/server`). */
function serverDir(): string {
  const instances = join(DATA_ROOT, 'data', 'instances');
  const [id] = existsSync(instances) ? readdirSync(instances) : [];
  return id ? join(instances, id, 'server') : join(instances, 'sem-instancia', 'server');
}

/** Se a porta está livre em 127.0.0.1. */
function portIsFree(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const probe = createServer();
    probe.once('error', () => {
      resolve(false);
    });
    probe.listen(port, '127.0.0.1', () => {
      probe.close(() => {
        resolve(true);
      });
    });
  });
}

function testButton() {
  return $('header.packhead .testbtn__main');
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** Abre "Testar como servidor…" pelo menu ▾. */
async function openServerTest(): Promise<void> {
  await dismissToasts();
  const more = $('header.packhead .testbtn__more');
  await more.waitForClickable();
  await more.click();
  const item = $('[role="menuitem"]*=Testar como servidor');
  await item.waitForClickable();
  await item.click();
}

/** O diálogo aberto, depois de o título dele aparecer. */
async function dialog(title: string) {
  const open = $('[role="dialog"]');
  await open.$(`h2=${title}`).waitForDisplayed({ timeout: 15_000 });
  return open;
}

describe('Testar como servidor (L-09; T27)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do servidor local pulado');
      this.skip();
    }
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack(
      {
        'config/vale.toml': 'dificuldade = 2\n',
        'server.properties': 'difficulty=hard\nserver-ip=0.0.0.0\n',
      },
      'conectar',
      'normal',
    );
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(FREE_MEMORY_FILE, { force: true });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(FREE_MEMORY_FILE, { force: true });
  });

  it('CA-T27-02: sem a EULA, pergunta com o link; "Agora não" não abre nada', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
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
    await waitHeading('Mods');

    await openServerTest();
    const eula = await dialog('Aceitar a EULA do Minecraft?');
    await expect(eula).toHaveText(expect.stringContaining('aka.ms/MinecraftEULA'));
    expect(await seriousViolations()).toEqual([]);
    await eula.$('button=Agora não').click();
    await eula.waitForDisplayed({ reverse: true });
    await expect(testButton()).toHaveText('Testar');
    // Nada foi aberto nem gravado.
    const file = join(serverDir(), 'eula.txt');
    expect(existsSync(file) && readFileSync(file, 'utf8').includes('eula=true')).toBe(false);
  });

  it('CA-T27-03: com pouca memória livre, o aviso aparece antes de abrir', async () => {
    writeFileSync(FREE_MEMORY_FILE, '2048');
    await openServerTest();
    const eula = await dialog('Aceitar a EULA do Minecraft?');
    await eula.$('button=Aceito a EULA').click();
    const options = await dialog('Testar como servidor');
    await expect(options).toHaveText(expect.stringContaining('Pode faltar memória.'), {
      wait: 15_000,
    });
    await expect(options).toHaveText(expect.stringContaining('Só o servidor'));
    await expect(options).toHaveText(
      expect.stringContaining('Só aceita conexões deste computador (127.0.0.1)'),
    );
    expect(await seriousViolations()).toEqual([]);
    await options.$('button=Cancelar').click();
    await options.waitForDisplayed({ reverse: true });
    await expect(testButton()).toHaveText('Testar');
    rmSync(FREE_MEMORY_FILE, { force: true });
  });

  it('CA-T27-01: servidor e jogo abrem, o jogo entra, o console do servidor recebe comandos e "Parar jogo e servidor" fecha os dois', async () => {
    await openServerTest();
    // A EULA já foi aceita: a pergunta não volta.
    const options = await dialog('Testar como servidor');
    await expect(options).not.toHaveText(expect.stringContaining('Pode faltar memória.'), {
      wait: 15_000,
    });
    const startButton = options.$('button=Abrir servidor e testar');
    await startButton.waitForEnabled({ timeout: 15_000 });
    await startButton.click();
    await waitHeading('Testando como servidor');

    const serverConsole = $('section[aria-label="Console do servidor"]');
    await browser.waitUntil(
      async () => (await serverConsole.getText()).includes('joined the game'),
      {
        timeout: 60_000,
        timeoutMsg: 'o jogo não entrou no servidor',
      },
    );
    await expect(serverConsole).toHaveText(expect.stringContaining('Done (0.512s)!'));
    expect(await serverConsole.getText()).not.toContain('jogo conectado');
    const sub = await $('main').$('.pagehead').getText();
    const port = Number(/porta (\d+)/.exec(sub)?.[1]);
    expect(port).toBeGreaterThan(0);

    const properties = readFileSync(join(serverDir(), 'server.properties'), 'utf8');
    for (const line of [
      'difficulty=hard',
      'server-ip=127.0.0.1',
      'online-mode=false',
      'enable-rcon=false',
    ]) {
      expect(properties.split('\n')).toContain(line);
    }
    expect(properties).toContain(`server-port=${String(port)}`);
    expect(readFileSync(join(serverDir(), 'eula.txt'), 'utf8')).toContain('eula=true');

    const command = $('input[placeholder^="Enviar comando ao servidor"]');
    await command.waitForEnabled({ timeout: 15_000 });
    await command.setValue('say oi');
    await browser.keys('Enter');
    await browser.waitUntil(
      async () => (await serverConsole.getText()).includes('Comando recebido: say oi'),
      { timeout: 15_000, timeoutMsg: 'o comando não chegou ao servidor' },
    );

    const showFor = await $('label=Mostrando').getAttribute('for');
    await $(`[id="${showFor ?? ''}"]`).selectByAttribute('value', 'game');
    const gameConsole = $('section[aria-label="Console do jogo"]');
    await gameConsole.waitForDisplayed();
    await browser.waitUntil(async () => (await gameConsole.getText()).includes('jogo conectado'), {
      timeout: 15_000,
      timeoutMsg: 'o console do jogo não mostrou o jogo',
    });
    expect(await gameConsole.getText()).not.toContain('Done (0.512s)!');
    expect(await seriousViolations()).toEqual([]);

    await dismissToasts();
    await $('main').$('button=Parar jogo e servidor').click();
    const confirm = $('[role="alertdialog"]');
    await confirm.waitForDisplayed();
    await confirm.$('button=Parar jogo e servidor').click();
    await waitHeading('Encerrado por você', 90_000);
    const result = $('section[aria-labelledby="server-result-title"]');
    await expect(result).toHaveText(expect.stringContaining('Entrou no servidor.'));
    await expect(result).toHaveText(expect.stringContaining('fechou normalmente'));
    await browser.waitUntil(() => portIsFree(port), {
      timeout: 15_000,
      timeoutMsg: `a porta ${String(port)} ficou presa`,
    });
    await expect(testButton()).toHaveText('Testar');
  });

  it('CA-T27-04: um mod de cliente derruba o servidor: "O servidor travou", o mod e o trecho do log', async () => {
    writePack(
      {
        'mods/zoomcliente-1.0.jar': 'jar de cliente marcado como dos dois lados\n',
        'mods/so-cliente.pw.toml': CLIENT_ONLY,
      },
      'conectar',
      'travar-cliente',
    );
    await openServerTest();
    const options = await dialog('Testar como servidor');
    await expect(options).toHaveText(expect.stringContaining('1 mod só de cliente fica de fora'), {
      wait: 15_000,
    });
    await options.$('label*=Só o servidor').click();
    const startButton = options.$('button=Abrir servidor e testar');
    await startButton.waitForEnabled({ timeout: 15_000 });
    await startButton.click();
    await waitHeading('O servidor travou', 90_000);
    const result = $('section[aria-labelledby="server-result-title"]');
    await expect(result).toHaveText(expect.stringContaining('é de cliente e derrubou o servidor.'));
    await expect(result).toHaveText(expect.stringContaining('invalid dist DEDICATED_SERVER'));
    await expect(result).toHaveText(expect.stringContaining('Travou antes de ficar pronto.'));
    expect(existsSync(join(serverDir(), 'mods', 'zoomcliente-1.0.jar'))).toBe(true);
    expect(existsSync(join(serverDir(), 'mods', 'so-cliente.jar'))).toBe(false);
    expect(await seriousViolations()).toEqual([]);
  });

  it('CA-T21-05: Configurações mostra o aceite e Revogar volta a "Ainda não aceita"', async () => {
    await dismissToasts();
    await $('header.packhead').$('a*=Meus packs').click();
    await $('h1=Meus packs').waitForDisplayed();
    const link = $('header.topbar').$('a*=Configurações');
    await link.waitForClickable();
    await link.click();
    const label = $('span=EULA do Minecraft');
    await label.waitForDisplayed({ timeout: 30_000 });
    const group = label.parentElement();
    await expect(group).toHaveText(expect.stringContaining('Aceita em'));
    await group.$('button=Revogar').click();
    await expect(group).toHaveText(expect.stringContaining('Ainda não aceita'), { wait: 15_000 });
    const settings = JSON.parse(readFileSync(SETTINGS_FILE, 'utf8')) as {
      eulaAcceptedAt?: string | null;
    };
    expect(settings.eulaAcceptedAt ?? null).toBeNull();
  });
});
