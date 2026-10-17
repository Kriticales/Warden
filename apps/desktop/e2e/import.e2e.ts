/**
 * Importar modpack no app real (P1-19; SPEC T24). O Modrinth é o servidor de fixtures dos E2E
 * (o Sodium gravado, `AANobbMI`/`SMxNOGZ6`); a CurseForge com chave e o `packwiz curseforge
 * import` são provados no Rust (`crates/warden-import/tests/import.rs`, CA-T24-03), com o packwiz
 * real e a CurseForge simulada.
 *
 * - `.mrpack` por "Abrir ou importar…" → "Escolher arquivo…": a revisão mostra referências, o
 *   jar local com o motivo, o item com lado desconhecido e o lixo que não entra; "Importar como
 *   pack novo" cria o pack (sem o lixo) e abre em Mods. Passa no axe.
 * - Zip da CurseForge sem chave: a frase da chave e "Abrir Configurações"; nada é criado.
 * - Pasta de instância do Prism por "Abrir ou importar…": vai para Importar modpack, mostra o
 *   perfil do teste e a pergunta das preferências; o pack novo não leva mundos nem logs e o
 *   perfil do teste fica com a memória da instância.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { storedZip } from './mock-server/jars';
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
const OUTSIDE = join(DATA_ROOT, 'fora', 'importar');
const PACKS = join(DATA_ROOT, 'packs');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
/** Onde o cofre de teste (arquivo) guarda a chave da CurseForge (`WARDEN_SECRET_BACKEND`). */
const KEY_FILE = join(DATA_ROOT, 'cofre-de-teste', 'curseforge-api-key');

/** O Sodium gravado nas fixtures do Modrinth (versão `SMxNOGZ6`). */
const SODIUM = {
  path: 'mods/sodium-fabric-0.8.13+mc1.21.1.jar',
  url: 'https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar',
  sha1: '003c114c85ca88ef3362e018deb6aca0c682d6a1',
  sha512:
    '8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e54e8be06d5ea5a9c268beb0e02073209d1f831a02dede851588b972367680853',
};

function write(path: string, content: string | Buffer): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content);
}

/** O `.mrpack` de teste: o Sodium (só cliente), um mod com `env` desconhecido, configs, lixo
 * de outro launcher e um jar solto que não está em loja nenhuma. */
function writeMrpack(path: string): void {
  const index = {
    formatVersion: 1,
    game: 'minecraft',
    versionId: '2.0.0',
    name: 'Vale do Cobre',
    summary: 'Modpack de teste da importação',
    dependencies: { minecraft: '1.21.1', 'fabric-loader': '0.16.5' },
    files: [
      {
        path: SODIUM.path,
        hashes: { sha1: SODIUM.sha1, sha512: SODIUM.sha512 },
        env: { client: 'required', server: 'unsupported' },
        downloads: [SODIUM.url],
        fileSize: 1574609,
      },
      {
        path: 'mods/estranho-1.0.jar',
        hashes: { sha1: 'a'.repeat(40), sha512: 'b'.repeat(128) },
        env: { client: 'unknown', server: 'unknown' },
        downloads: ['https://cdn.modrinth.com/data/ESTRANHO/versions/V1/estranho-1.0.jar'],
        fileSize: 10,
      },
    ],
  };
  write(
    path,
    storedZip({
      'modrinth.index.json': JSON.stringify(index, null, 2),
      'xmcl.json': '{\n    "server": null,\n    "hideLauncher": false\n}',
      'overrides/config/sodium-options.json': '{ "quality": {} }\n',
      'overrides/.mixin.out/class/A.class': 'depuração de mixin',
      'overrides/config/sodium-options-1.json.bak': 'cópia velha',
      'overrides/mods/caseiro.jar': 'jar feito em casa',
    }),
  );
}

function writeCurseforgeZip(path: string): void {
  write(
    path,
    storedZip({
      'manifest.json': JSON.stringify({
        minecraft: { version: '1.20.1', modLoaders: [{ id: 'forge-47.3.0', primary: true }] },
        manifestType: 'minecraftModpack',
        manifestVersion: 1,
        name: 'Cozy Create',
        version: '2.3.0',
        author: 'Autora',
        files: [{ projectID: 238222, fileID: 4712345, required: true }],
        overrides: 'overrides',
      }),
      'overrides/config/a.toml': 'a = 1\n',
    }),
  );
}

/** Instância do Prism com memória própria, mundos, logs e preferências. */
function writePrismInstance(root: string): void {
  rmSync(root, { recursive: true, force: true });
  write(
    join(root, 'instance.cfg'),
    '[General]\nname=Bosque no Prism\nOverrideMemory=true\nMaxMemAlloc=6144\nOverrideJavaArgs=true\nJvmArgs=-XX:+UseG1GC\n',
  );
  write(
    join(root, 'mmc-pack.json'),
    JSON.stringify({
      components: [
        { uid: 'net.minecraft', version: '1.21.1' },
        { uid: 'net.fabricmc.fabric-loader', version: '0.16.5' },
      ],
      formatVersion: 1,
    }),
  );
  const game = join(root, '.minecraft');
  write(join(game, 'config', 'bosque.toml'), 'arvores = 3\n');
  write(join(game, 'options.txt'), 'lang:pt_br\n');
  write(join(game, 'saves', 'Mundo', 'level.dat'), 'mundo');
  write(join(game, 'logs', 'latest.log'), '[main] log do jogo\n');
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.saveScreenshot(join(SCREENSHOTS, `import-${name}.png`));
}

/** Se o passo falhar, o erro diz qual foi e o que a tela mostrava (o CI não guarda capturas). */
async function step(name: string, action: () => Promise<void>): Promise<void> {
  try {
    await action();
  } catch (error) {
    const body = await browser.execute(() =>
      (document.querySelector('main') ?? document.body).innerText.slice(0, 1500),
    );
    throw new Error(`passo "${name}" falhou: ${String(error)}\ntela: ${body}`);
  }
}

/** Todos os arquivos de uma pasta, relativos, com `/`. */
function filesOf(root: string, prefix = ''): string[] {
  return readdirSync(join(root, prefix), { withFileTypes: true }).flatMap((entry) => {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.name === '.git') return [];
    return entry.isDirectory() ? filesOf(root, relative) : [relative];
  });
}

async function backToPacks(): Promise<void> {
  await dismissToasts();
  await $('a*=Meus packs').click();
  await $('h1=Meus packs').waitForDisplayed({ timeout: 20_000 });
}

async function waitImportEnabled(): Promise<void> {
  const button = $('button=Importar como pack novo');
  await button.waitForDisplayed({ timeout: 60_000 });
  await browser.waitUntil(async () => button.isEnabled(), {
    timeout: 20_000,
    timeoutMsg: 'Importar como pack novo não ficou disponível',
  });
}

describe('Importar modpack (P1-19; T24)', () => {
  before(async () => {
    rmSync(OUTSIDE, { recursive: true, force: true });
    rmSync(PACKS, { recursive: true, force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(KEY_FILE, { force: true });
    mkdirSync(OUTSIDE, { recursive: true });
    writeFileSync(PICK_FILE, '');
    write(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    writeFileSync(PICK_FILE, '');
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACKS, { recursive: true, force: true });
  });

  it('.mrpack: revisão, importação sem o lixo e o pack aberto em Mods; passa no axe', async () => {
    const origin = join(OUTSIDE, 'Vale do Cobre 2.0.0.mrpack');
    writeMrpack(origin);
    const before = readFileSync(origin);
    await step('Meus packs vazio', async () => {
      await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 20_000 });
    });
    await step('Abrir ou importar… → Escolher arquivo…', async () => {
      writeFileSync(PICK_FILE, origin);
      await openOrImport('arquivo');
      await $('h1=Importar modpack').waitForDisplayed({ timeout: 20_000 });
    });
    await step('revisão', async () => {
      await waitImportEnabled();
      const main = $('main');
      await expect(main).toHaveText(
        expect.stringContaining('Vale do Cobre 2.0.0.mrpack · Minecraft 1.21.1 · Fabric 0.16.5'),
      );
      await expect(main).toHaveText(expect.stringContaining('2 viram referência ao Modrinth'));
      await expect(main).toHaveText(expect.stringContaining('1 jar fica como arquivo local'));
      await expect(main).toHaveText(expect.stringContaining('1 item para revisar'));
      await expect(main).toHaveText(expect.stringContaining('.mixin.out/'));
      await expect(main).toHaveText(expect.stringContaining('xmcl.json'));
      // A pasta do usuário não aparece na tela.
      await expect(main).not.toHaveText(expect.stringContaining(OUTSIDE));
    });
    await step('ver a lista do que fica para revisar', async () => {
      await dismissToasts();
      await $('li[data-linha="revisar"]').$('summary=Ver lista').click();
      await $('span*=lado desconhecido no modpack').waitForDisplayed();
    });
    await step('axe', async () => {
      expect(await seriousViolationSummaries()).toEqual([]);
    });
    await screenshot('01-revisao');
    await step('importar', async () => {
      await dismissToasts();
      await $('button=Importar como pack novo').click();
      await $('nav[aria-label="Seções do pack"]').waitForDisplayed({ timeout: 90_000 });
    });
    await screenshot('02-pack-aberto');
    const root = join(PACKS, 'vale-do-cobre');
    const files = filesOf(root);
    expect(files).toContain('mods/sodium.pw.toml');
    expect(files).toContain('mods/caseiro.jar');
    expect(files).toContain('config/sodium-options.json');
    expect(files.filter((file) => /mixin|xmcl|\.bak$/.test(file))).toEqual([]);
    const sodium = readFileSync(join(root, 'mods', 'sodium.pw.toml'), 'utf8');
    expect(sodium).toContain('side = "client"');
    expect(sodium).toContain('[update.modrinth]');
    expect(readFileSync(join(root, '.warden', 'project.toml'), 'utf8')).toContain('[import]');
    // O arquivo de origem não mudou.
    expect(readFileSync(origin).equals(before)).toBe(true);
    await backToPacks();
    // A lista de Meus packs já mostra o pack novo (a importação invalida a lista).
    await $('button[aria-label="Abrir Vale do Cobre"]').waitForDisplayed({ timeout: 20_000 });
  });

  it('zip da CurseForge sem chave: explica, oferece Configurações e não cria nada', async () => {
    const origin = join(OUTSIDE, 'Cozy Create 2.3.0.zip');
    writeCurseforgeZip(origin);
    const packsBefore = readdirSync(PACKS);
    await step('Abrir ou importar… → Escolher arquivo…', async () => {
      await dismissToasts();
      writeFileSync(PICK_FILE, origin);
      await openOrImport('arquivo');
      await $('h1=Importar modpack').waitForDisplayed({ timeout: 20_000 });
    });
    await step('mensagem da chave', async () => {
      await $(
        'div*=Para importar um modpack da CurseForge, o Warden precisa da sua chave da CurseForge.',
      ).waitForDisplayed({ timeout: 30_000 });
      await $('button=Abrir Configurações').waitForDisplayed();
      expect(await $('button=Importar como pack novo').isExisting()).toBe(false);
    });
    await screenshot('03-sem-chave');
    expect(readdirSync(PACKS)).toEqual(packsBefore);
    await backToPacks();
  });

  it('instância do Prism por "Abrir ou importar…": perfil do teste e sem mundos nem logs', async () => {
    const origin = join(OUTSIDE, 'Bosque no Prism');
    writePrismInstance(origin);
    await step('Abrir ou importar… com a pasta da instância', async () => {
      await dismissToasts();
      writeFileSync(PICK_FILE, origin);
      await openOrImport();
      await $('h1=Importar modpack').waitForDisplayed({ timeout: 20_000 });
    });
    await step('revisão da instância', async () => {
      await waitImportEnabled();
      await $('h2=Perfil do teste').waitForDisplayed();
      await expect($('main')).toHaveText(expect.stringContaining('Memória: 6144 MB'));
      await $('h2=Levar suas preferências para o pack?').waitForDisplayed();
    });
    await screenshot('04-prism');
    await step('importar', async () => {
      await dismissToasts();
      await $('button=Importar como pack novo').click();
      await $('nav[aria-label="Seções do pack"]').waitForDisplayed({ timeout: 90_000 });
    });
    const root = join(PACKS, 'bosque-no-prism');
    const files = filesOf(root);
    expect(files).toContain('config/bosque.toml');
    expect(files.filter((file) => /^(saves|logs)\/|options\.txt$/.test(file))).toEqual([]);
    expect(existsSync(join(origin, '.minecraft', 'saves', 'Mundo', 'level.dat'))).toBe(true);
    const registry = readFileSync(REGISTRY_FILE, 'utf8');
    expect(registry).toContain('Do modpack importado');
    expect(registry).toContain('6144');
  });
});
