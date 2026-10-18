/**
 * Busca do culpado no app real (SPEC T25; ROADMAP D-12), com o jogo simulado da
 * `warden-launcher` no modo `culpado`: ele entra no mundo como o Minecraft, mas falha ao entrar
 * ("Couldn't place player in world" e a exceção, sem fechar) quando o jar `charlie` está em
 * `mods/` da instância da rodada; e só abre o mundo do Quick Play se `saves/<mundo>/level.dat`
 * existir na rodada (escreve a primeira linha dele no log). O servidor simulado da
 * `warden-server` faz o papel do servidor vanilla que gera o mundo de teste (D-12 item 7):
 * cria `world/level.dat` com a semente do `server.properties`.
 *
 * - O menu ▾ do Testar abre "Encontrar o mod culpado"; na primeira busca, o diálogo explica o
 *   mundo de teste e pede a EULA (D18); com o aceite, a busca gera o mundo sem mods, roda no
 *   modo da tela do teste, com o botão do cabeçalho em "Buscando o culpado: ver progresso",
 *   pausa depois da rodada, continua do estado guardado e acha o `charlie.jar` com confiança
 *   alta; cada rodada abriu uma cópia do mundo gerado com a semente fixa.
 * - "Começar de novo" e "Cancelar busca" no meio: o diálogo não pede a EULA de novo (o mundo
 *   já existe), o resultado parcial aparece, a instância da busca some, e o pack, a instância de
 *   teste e o mundo do usuário não mudam.
 *
 * A estabilidade de cada rodada é encurtada pelo `wdio.conf.ts`
 * (`WARDEN_E2E_BISECT_STABILITY_MS`, só em build de debug). Com `WARDEN_E2E_SCREENSHOTS=<pasta>`,
 * grava as capturas das telas.
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
import { openOrImport } from './open-or-import';
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
const PACK = join(DATA_ROOT, 'fora', 'pack-culpado');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const MODS = ['alfa', 'bravo', 'charlie', 'delta', 'echo'];
/** A semente fixa do mundo de teste (`bisect_session::world::WORLD_SEED`). */
const WORLD_SEED = '20261004';

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um jar Fabric com o `fabric.mod.json` (`delta` exige `alfa`). */
function jar(id: string): Buffer {
  const depends = id === 'delta' ? '{"alfa":"*"}' : '{}';
  return storedZip({
    'fabric.mod.json': `{"schemaVersion":1,"id":"${id}","version":"1.0","depends":${depends}}`,
  });
}

/** Pack Fabric 1.20.1 com cinco mods locais e o jogo simulado que falha com o `charlie`. */
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
      'name = "Vale do Culpado"',
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
      'fabric = "0.16.10"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, '.warden-fake-game'), 'culpado 600 charlie');
}

/** Os packs registrados. */
function registeredPacks(): { id: string; path: string }[] {
  if (!existsSync(REGISTRY_FILE)) return [];
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs?: { id: string; path: string }[];
  };
  return registry.packs ?? [];
}

function packId(): string {
  const pack = registeredPacks().find((entry) => entry.path === PACK);
  if (!pack) throw new Error('pack do culpado não registrado');
  return pack.id;
}

function instanceRoot(): string {
  return join(DATA_ROOT, 'data', 'instances', packId());
}

/** Um mundo do usuário na instância de teste, salvo com todos os mods: a busca nunca o usa. */
function writeUserWorld(): void {
  const world = join(instanceRoot(), 'minecraft', 'saves', 'Mundo de teste');
  mkdirSync(join(world, 'region'), { recursive: true });
  writeFileSync(join(world, 'level.dat'), 'semente=mundo do usuário');
  writeFileSync(join(world, 'region', 'r.0.0.mca'), 'chunks');
}

/** Os `output.log` das rodadas guardadas (`state/bisect-logs/<busca>/round-<n>/`). */
function roundLogs(): string[] {
  const root = join(instanceRoot(), 'state', 'bisect-logs');
  if (!existsSync(root)) return [];
  return readdirSync(root).flatMap((search) =>
    readdirSync(join(root, search))
      .map((round) => join(root, search, round, 'output.log'))
      .filter((file) => existsSync(file)),
  );
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
 * Abre o diálogo pelo menu ▾ e começa a busca "Trava ao entrar no mundo". Na primeira busca
 * (`firstSearch`), o diálogo pede a EULA para gerar o mundo de teste, e o teste aceita.
 */
async function startFromMenu(firstSearch: boolean): Promise<void> {
  await clickStable($('header.packhead .testbtn__more'));
  await clickStable($('[role="menuitem"]*=Encontrar o mod culpado…'));
  const dialog = $('[role="dialog"]');
  await dialog.$('h2=Encontrar o mod culpado').waitForDisplayed({ timeout: 30_000 });
  await dialog.$('span*=5 mods entram na busca.').waitForDisplayed({ timeout: 30_000 });
  const eula = dialog.$('.alert__title=Para entrar no mundo, o Warden cria um mundo de teste');
  if (firstSearch) {
    await eula.waitForDisplayed({ timeout: 10_000 });
    await expect(dialog.$('input[type="radio"][name="bisect-symptom"][disabled]')).toExist();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d12-00-dialogo-eula');
    await clickStable(dialog.$('button=Aceito a EULA'));
    await eula.waitForDisplayed({ reverse: true, timeout: 10_000 });
    await dialog.$('span*=Antes da primeira rodada, o Warden cria').waitForDisplayed();
  } else {
    await dialog.$('span*=Cada rodada entra numa cópia').waitForDisplayed();
    expect(await eula.isExisting()).toBe(false);
  }
  expect(await seriousViolationSummaries()).toEqual([]);
  await screenshot(firstSearch ? 'd12-00-dialogo' : 'd12-00-dialogo-de-novo');
  await clickStable(dialog.$('label*=Trava ao entrar no mundo'));
  await clickStable(dialog.$('button=Começar a busca'));
  await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
}

describe('Busca do culpado (D-12; SPEC T25)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E da busca do culpado pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    writePack();
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(REGISTRY_FILE, { force: true });
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

  it('acha o mod culpado pelo menu ▾, no modo da tela do teste', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    writeFileSync(PICK_FOLDER, PACK);
    await openOrImport();
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
    writeUserWorld();

    await startFromMenu(true);
    // O aceite fica nas Configurações (o mesmo do "Testar como servidor").
    const settings = JSON.parse(readFileSync(SETTINGS_FILE, 'utf8')) as {
      eulaAcceptedAt?: string | null;
    };
    expect(settings.eulaAcceptedAt).toEqual(expect.any(String));
    await waitHeading('Buscando o mod culpado');
    await expect($('header.packhead .testbtn__main')).toHaveText(
      expect.stringContaining('Buscando o culpado: ver progresso'),
    );
    await $('ol[aria-label="Rodadas da busca"]').waitForDisplayed();
    await screenshot('d12-01-buscando');

    // Pausar espera a rodada atual; a busca pausada continua do estado guardado. Pausada, a
    // tela fica parada e o axe mede (rodando, as rodadas mudam a tela a cada segundo).
    await clickStable($('button=Pausar depois desta rodada'));
    await $('.alert__title=Busca pausada.').waitForDisplayed({ timeout: 60_000 });
    await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d12-02-pausada');
    await clickStable($('button=Continuar a busca'));

    await waitHeading('Culpado encontrado: charlie.jar', 180_000);
    await expect($('main')).toHaveText(
      expect.stringContaining('Alta: confirmado em 2 testes de controle'),
    );
    await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    await clickStable($('summary*=Ver as'));
    const table = $('table*=Rodada');
    await table.waitForDisplayed();
    await expect(table).toHaveText(expect.stringContaining('Pack inteiro'));
    await expect(table).toHaveText(expect.stringContaining('Travou igual'));
    await expect(table).toHaveText(expect.stringContaining('Passou'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d12-03-resultado');
    // A instância da busca é apagada no fim.
    expect(existsSync(join(instanceRoot(), 'bisect'))).toBe(false);
    expect(existsSync(join(instanceRoot(), 'bisect-base'))).toBe(false);
    // O mundo de teste foi gerado sem mods, com a semente fixa, e ficou como modelo; a pasta do
    // servidor sumiu; cada rodada abriu uma cópia dele, nunca o mundo do usuário.
    const template = join(instanceRoot(), 'state', 'bisect-world');
    expect(readFileSync(join(template, 'world', 'level.dat'), 'utf8').trim()).toBe(
      `semente=${WORLD_SEED}`,
    );
    expect(JSON.parse(readFileSync(join(template, 'mundo.json'), 'utf8'))).toEqual({
      minecraft: '1.20.1',
      seed: Number(WORLD_SEED),
    });
    expect(existsSync(join(instanceRoot(), 'bisect-world-server'))).toBe(false);
    const logs = roundLogs();
    expect(logs.length).toBeGreaterThan(3);
    for (const log of logs) {
      expect(readFileSync(log, 'utf8')).toContain(`Mundo de teste: semente=${WORLD_SEED}`);
    }
  });

  it('cancelar no meio guarda o parcial e não muda o pack, a instância de teste nem o mundo do usuário', async () => {
    const before = {
      pack: treeHash(PACK),
      test: treeHash(join(instanceRoot(), 'minecraft')),
    };
    await startFromMenu(false);
    await waitHeading('Buscando o mod culpado');
    // Espera uma rodada terminar e a seguinte começar.
    await $('h2*=Agora: rodada 1').waitForDisplayed({ timeout: 60_000 });

    await clickStable($('button=Cancelar busca'));
    const confirm = $('[role="alertdialog"]');
    await confirm.$('h2=Cancelar a busca do culpado?').waitForDisplayed();
    await clickStable(confirm.$('button=Cancelar busca'));
    await waitHeading('Busca cancelada', 30_000);
    await expect($('main')).toHaveText(expect.stringContaining('O culpado está entre estes'));
    await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d12-04-cancelada');
    await browser.waitUntil(() => !existsSync(join(instanceRoot(), 'bisect')), {
      timeout: 10_000,
      timeoutMsg: 'a instância da busca não foi apagada',
    });
    expect(treeHash(PACK)).toBe(before.pack);
    expect(treeHash(join(instanceRoot(), 'minecraft'))).toBe(before.test);
  });
});
