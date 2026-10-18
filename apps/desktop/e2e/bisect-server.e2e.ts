/**
 * Busca do culpado com o servidor local no app real (SPEC T25 e T27; ROADMAP D-13; CA-T25-06),
 * com processos simulados: o jogo simulado da `warden-launcher` no modo `entrar-servidor`
 * (carrega e entra no servidor por `--server`/`--port`, como antes de 1.20) e o servidor
 * simulado da `warden-server` no modo `culpado` (o jogador não consegue entrar quando o jar
 * `charlie` está em `mods/` do servidor da rodada). O app não baixa Java, jogo nem servidor;
 * todo o resto é o de verdade.
 *
 * - Num pack Forge 1.16.5, "Trava ao entrar no mundo" explica que precisa do servidor local e
 *   pede antes de abrir (com a memória e a EULA); com o aceite, a busca gera o mundo de teste
 *   sem mods (D-12, o mesmo servidor simulado no papel do vanilla), cada rodada sobe o servidor
 *   em 127.0.0.1 com uma cópia descartável desse mundo, o jogo entra nele, o servidor fecha ao
 *   fim da rodada e a busca acha o `charlie.jar` pelo log do servidor ("Ver log" com os dois
 *   lados).
 * - Cancelar com o servidor e o jogo abertos fecha os dois e apaga as pastas da busca; o pack, o
 *   mundo do usuário, o mundo de teste modelo e a pasta do "Testar como servidor" não mudam.
 *
 * A estabilidade de cada rodada é encurtada pelo `wdio.conf.ts`
 * (`WARDEN_E2E_BISECT_STABILITY_MS`). Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas.
 */
import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { join, relative } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { storedZip } from './mock-server/jars';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-culpado-servidor');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const MODS = ['alfa', 'bravo', 'charlie', 'delta', 'echo'];

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um jar Forge com o `META-INF/mods.toml` (`delta` exige `alfa`). */
function jar(id: string): Buffer {
  const depends =
    id === 'delta'
      ? '\n[[dependencies.delta]]\nmodId="alfa"\nmandatory=true\nversionRange="[1,)"\nordering="NONE"\nside="BOTH"\n'
      : '';
  return storedZip({
    'META-INF/mods.toml': `modLoader="javafml"\nloaderVersion="[36,)"\nlicense="MIT"\n\n[[mods]]\nmodId="${id}"\nversion="1.0"\n${depends}`,
  });
}

/** Pack Forge 1.16.5 com cinco mods locais; o servidor simulado barra a entrada com o `charlie`. */
function writePack(): void {
  mkdirSync(join(PACK, 'mods'), { recursive: true });
  const entries: string[] = [];
  for (const id of MODS) {
    const bytes = jar(id);
    writeFileSync(join(PACK, 'mods', `${id}.jar`), bytes);
    entries.push(`[[files]]\nfile = "mods/${id}.jar"\nhash = "${sha256(bytes)}"\n`);
  }
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale do Servidor Culpado"',
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
      'minecraft = "1.16.5"',
      'forge = "36.2.39"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, '.warden-fake-game'), 'entrar-servidor 300');
  writeFileSync(join(PACK, '.warden-fake-server'), 'culpado charlie');
}

/** Os packs registrados. */
function registeredPacks(): { id: string; path: string }[] {
  if (!existsSync(REGISTRY_FILE)) return [];
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs?: { id: string; path: string }[];
  };
  return registry.packs ?? [];
}

function instanceRoot(): string {
  const pack = registeredPacks().find((entry) => entry.path === PACK);
  if (!pack) throw new Error('pack do culpado com servidor não registrado');
  return join(DATA_ROOT, 'data', 'instances', pack.id);
}

/** Um mundo do usuário na instância de teste (a busca nunca o usa). */
function writeUserWorld(): string {
  const world = join(instanceRoot(), 'minecraft', 'saves', 'Mundo de teste');
  mkdirSync(join(world, 'region'), { recursive: true });
  writeFileSync(join(world, 'level.dat'), 'nbt');
  writeFileSync(join(world, 'region', 'r.0.0.mca'), 'chunks');
  return world;
}

/** Hash de todos os arquivos de uma pasta (caminho e conteúdo). */
function treeHash(dir: string): string {
  const files: string[] = [];
  const walk = (current: string) => {
    if (!existsSync(current)) return;
    for (const name of readdirSync(current)) {
      const path = join(current, name);
      if (statSync(path).isDirectory()) walk(path);
      else files.push(path);
    }
  };
  walk(dir);
  const hash = createHash('sha256');
  for (const path of files.sort()) {
    hash.update(relative(dir, path));
    hash.update(readFileSync(path));
  }
  return hash.digest('hex');
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** Clica depois de fechar os avisos e de o alvo parar de mexer. */
async function clickStable(element: ReturnType<typeof $>): Promise<void> {
  await dismissToasts();
  await element.waitForClickable({ timeout: 30_000 });
  await element.waitForStable({ timeout: 10_000 });
  await element.click();
}

/**
 * Abre o diálogo pelo menu ▾, escolhe "Trava ao entrar no mundo" e aceita o servidor local (com
 * a EULA, se ainda faltar); começa a busca.
 */
async function startWithServer(withEula: boolean): Promise<void> {
  await clickStable($('header.packhead .testbtn__more'));
  await clickStable($('[role="menuitem"]*=Encontrar o mod culpado…'));
  const dialog = $('[role="dialog"]');
  await dialog.$('h2=Encontrar o mod culpado').waitForDisplayed({ timeout: 30_000 });
  await dialog.$('span*=5 mods entram na busca.').waitForDisplayed({ timeout: 30_000 });
  await clickStable(dialog.$('label*=Trava ao entrar no mundo'));
  await dialog
    .$('.alert__title=Para este problema, o Warden precisa abrir um servidor neste computador.')
    .waitForDisplayed({ timeout: 30_000 });
  await expect(dialog).toHaveText(expect.stringContaining('No Minecraft 1.16.5'));
  await expect(dialog).toHaveText(expect.stringContaining('Livre agora'));
  if (withEula) {
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d13-00-pedido-eula');
    await clickStable(dialog.$('button=Aceito a EULA'));
  }
  await clickStable(dialog.$('button=Entendi, pode abrir'));
  await dialog.$('p=Combinado: cada rodada abre o servidor local.').waitForDisplayed();
  // A primeira busca cria o mundo de teste (D-12); a cópia dele vai para o servidor.
  if (withEula) {
    await dialog
      .$('span*=Antes da primeira rodada, o Warden cria o “Mundo de teste”')
      .waitForDisplayed();
  }
  if (withEula) {
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d13-01-pedido-aceito');
  }
  await clickStable(dialog.$('button=Começar a busca'));
  await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
}

describe('Busca do culpado com o servidor local (D-13; CA-T25-06)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E da busca com servidor pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(REGISTRY_FILE, { force: true });
    // Sem a EULA aceita: o pedido da busca pergunta.
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // A instância deste pack sai junto: os outros E2E contam as instâncias.
    for (const pack of registeredPacks()) {
      rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
  });

  it('pede antes de abrir, sobe o servidor em cada rodada e acha o mod pelo log do servidor', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    writeFileSync(PICK_FOLDER, PACK);
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
    const world = writeUserWorld();
    const before = { pack: treeHash(PACK), world: treeHash(world) };

    await startWithServer(true);
    await waitHeading('Buscando o mod culpado');
    await waitHeading('Culpado encontrado: charlie.jar', 180_000);
    await expect($('main')).toHaveText(
      expect.stringContaining('Alta: confirmado em 2 testes de controle'),
    );
    await expect($('main')).toHaveText(expect.stringContaining('travaram no servidor'));
    await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    // A EULA ficou guardada nas Configurações.
    expect(readFileSync(SETTINGS_FILE, 'utf8')).toContain('eulaAcceptedAt');

    await clickStable($('summary*=Ver as'));
    const table = $('table*=Rodada');
    await table.waitForDisplayed();
    await expect(table).toHaveText(expect.stringContaining('Pack inteiro'));
    await expect(table).toHaveText(expect.stringContaining('no servidor'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d13-02-resultado');

    // "Ver log" da rodada 0 (pack inteiro): o jogo e o servidor, um de cada vez.
    await clickStable(table.$('button*=Ver o log da rodada 0'));
    const log = $('[role="dialog"]');
    await log.$('select').waitForDisplayed();
    await expect(log).toHaveText(expect.stringContaining('entra pelo --server'));
    await log.$('select').selectByAttribute('value', 'server');
    await browser.waitUntil(
      async () => (await log.getText()).includes("Couldn't place player in world"),
      { timeout: 10_000, timeoutMsg: 'o log do servidor não apareceu' },
    );
    await expect(log).toHaveText(
      expect.stringContaining('Preparing level "world" (mundo existente)'),
    );
    await expect(log).toHaveText(expect.stringContaining('Stopping the server'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d13-03-log-dos-dois-lados');
    await browser.keys('Escape');
    await log.waitForDisplayed({ reverse: true });

    // Nada da busca fica no disco; o pack, o mundo do usuário e o "Testar como servidor",
    // intactos. O mundo de teste modelo (D-12) fica para as próximas buscas.
    expect(
      readFileSync(join(instanceRoot(), 'state', 'bisect-world', 'world', 'level.dat'), 'utf8'),
    ).toContain('semente=');
    expect(existsSync(join(instanceRoot(), 'bisect-world-server'))).toBe(false);
    expect(existsSync(join(instanceRoot(), 'bisect'))).toBe(false);
    expect(existsSync(join(instanceRoot(), 'bisect-base'))).toBe(false);
    expect(existsSync(join(instanceRoot(), 'server'))).toBe(false);
    expect({ pack: treeHash(PACK), world: treeHash(world) }).toEqual(before);
  });

  it('cancelar com o servidor e o jogo abertos fecha os dois e apaga as pastas da busca', async () => {
    // O jogo nunca entra: a rodada fica aberta até cancelar.
    writeFileSync(join(PACK, '.warden-fake-game'), 'filhos 1');
    const world = join(instanceRoot(), 'minecraft', 'saves', 'Mundo de teste');
    const model = join(instanceRoot(), 'state', 'bisect-world', 'world');
    const before = { world: treeHash(world), model: treeHash(model) };
    await startWithServer(false);
    await waitHeading('Buscando o mod culpado');
    await $('p*=Servidor local pronto em 127.0.0.1').waitForDisplayed({ timeout: 60_000 });
    await browser.waitUntil(
      () => existsSync(join(instanceRoot(), 'bisect', 'server', 'world', 'level.dat')),
      { timeout: 30_000, timeoutMsg: 'a cópia do mundo não chegou ao servidor da rodada' },
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d13-04-rodada-com-servidor');

    await clickStable($('button=Cancelar busca'));
    const confirm = $('[role="alertdialog"]');
    await confirm.$('h2=Cancelar a busca do culpado?').waitForDisplayed();
    await clickStable(confirm.$('button=Cancelar busca'));
    await waitHeading('Busca cancelada', 30_000);
    await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    await browser.waitUntil(() => !existsSync(join(instanceRoot(), 'bisect')), {
      timeout: 10_000,
      timeoutMsg: 'as pastas da busca não foram apagadas',
    });
    expect(existsSync(join(instanceRoot(), 'bisect-base'))).toBe(false);
    expect({ world: treeHash(world), model: treeHash(model) }).toEqual(before);
  });
});
