/**
 * Editor de scripts KubeJS no app real (C-07; SPEC T26), com o jogo simulado da
 * `warden-launcher` (`WardenFakeGame`, como no `test.e2e.ts`) e um servidor web do KubeJS 7
 * simulado em `127.0.0.1`.
 *
 * - CA-T26-01: num pack Forge 1.20.1 com KubeJS 6, `kubejs/server_scripts/receitas.js` abre com
 *   "KubeJS 6 · Forge 1.20.1", os trechos prontos da versão e, ao digitar `'minecraft:copper_`,
 *   o autocompletar lista `minecraft:copper_ingot` (do jar do Minecraft instalado).
 * - Sem jogo aberto, Recarregar no jogo fica indisponível com o motivo.
 * - CA-T26-04: com o jogo aberto, Recarregar scripts (tela do teste) e Recarregar no jogo
 *   (editor) mostram `/kubejs reload server_scripts` com Copiar.
 * - CA-T26-02: depois do log real do KubeJS 6 (`receitas.js#16`), o painel mostra o erro na
 *   linha 16 e clicar leva à linha.
 * - CA-T26-03: com o KubeJS 7, Recarregar no jogo chama `POST /api/reload/server` com o token
 *   lido da instância de teste; o token não aparece na tela, nos registros nem no pack.
 * - Com o jogo aberto, salvar um script **no pack** grava no pack, copia para a instância de
 *   teste e recarrega (Salvar e recarregar); uma mudança feita direto na instância não é
 *   apagada sem a pessoa escolher Substituir.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
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
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';
import type { ChainablePromiseElement } from 'webdriverio';

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
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-scripts');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const MINECRAFT_JAR = join(DATA_ROOT, 'data', 'shared', 'versions', '1.20.1', '1.20.1.jar');
const RECEITAS = 'kubejs/server_scripts/receitas.js';
const REAL_LOGS = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  'crates',
  'warden-scripts',
  'tests',
  'reais',
  'kubejs-6-fabric-1.20.1',
);
const TOKEN = 'dG9rZW4tZG8tc2Vydmlkb3Itd2ViLWRvLWt1YmVqcy1lMmU';

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

function kubejsJar(version: string): Buffer {
  return storedZip({
    'META-INF/mods.toml': [
      'modLoader = "javafml"',
      'loaderVersion = "[47,)"',
      'license = "LGPL-3.0"',
      '[[mods]]',
      'modId = "kubejs"',
      `version = "${version}"`,
      'displayName = "KubeJS"',
      '',
    ].join('\n'),
  });
}

/** O pack Forge 1.20.1 com o KubeJS como arquivo local e o script real. */
function writePack(kubejsVersion: string): void {
  mkdirSync(PACK, { recursive: true });
  const files: Record<string, string | Buffer> = {
    'mods/kubejs-forge.jar': kubejsJar(kubejsVersion),
    [RECEITAS]: readFileSync(join(REAL_LOGS, RECEITAS)),
  };
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, data]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, data);
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(data)}"\n`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale dos Scripts"',
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
      'forge = "47.3.0"',
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
  // O jogo simulado fica aberto, com um processo filho, até "Parar jogo".
  writeFileSync(join(PACK, '.warden-fake-game'), 'filhos 1');
}

/** A instância de teste deste pack, pelo registro de packs. */
function instanceGameDir(): string {
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs?: { id: string; path: string }[];
  };
  const record = (registry.packs ?? []).find((pack) => pack.path.includes('pack-scripts'));
  if (!record) throw new Error('pack dos scripts não está no registro');
  return join(DATA_ROOT, 'data', 'instances', record.id, 'minecraft');
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
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

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** Abre um arquivo em Configs pelo filtro da árvore. */
async function openFile(name: string): Promise<void> {
  await dismissToasts();
  const filter = $('input[aria-label="Filtrar por nome de arquivo"]');
  await filter.waitForDisplayed();
  await filter.setValue(name);
  const item = $('[role="tree"]').$(`[role="treeitem"]*=${name}`);
  await item.waitForDisplayed({ timeout: 30_000 });
  await item.click();
  await $(`[aria-label="Conteúdo de ${RECEITAS}"]`).waitForDisplayed({ timeout: 30_000 });
}

async function goToConfigs(): Promise<void> {
  await dismissToasts();
  await $('nav[aria-label="Seções do pack"]').$('a*=Configs').click();
  await waitHeading('Configs');
}

/**
 * Digita tecla a tecla, conferindo cada uma: o driver do WebKitGTK às vezes perde a segunda de
 * duas teclas iguais seguidas (`pp`) quando vão no mesmo pedido.
 */
async function typeSlowly(text: string): Promise<void> {
  for (const key of text) {
    const before = await browser.execute(
      () => document.querySelector('.cm-content')?.textContent.length ?? 0,
    );
    await browser.keys(key);
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => document.querySelector('.cm-content')?.textContent.length ?? 0,
        )) > before,
      { timeout: 5000, timeoutMsg: `a tecla "${key}" não chegou ao editor` },
    );
  }
}

/** Espera o aviso (`.alert`) que contém o texto (no título ou no corpo) e o devolve. */
async function waitAlert(text: string): Promise<ChainablePromiseElement> {
  const alert = $(
    `//*[contains(concat(" ", normalize-space(@class), " "), " alert ") and contains(., "${text}")]`,
  );
  await alert.waitForDisplayed({ timeout: 30_000, timeoutMsg: `aviso "${text}" não apareceu` });
  return alert;
}

/** O texto do rodapé do editor ("… linha 16, coluna 1"). */
async function footer(): Promise<string> {
  return $('.editor__foot').getText();
}

/** Todos os arquivos de texto de uma pasta (para a varredura do token). */
function allText(dir: string): string {
  if (!existsSync(dir)) return '';
  let out = '';
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) out += allText(path);
    else if (stat.size < 8 * 1024 * 1024) out += readFileSync(path, 'latin1');
  }
  return out;
}

interface FakeKubejs {
  server: Server;
  port: number;
  reloads: string[];
}

/** O servidor web do KubeJS 7 simulado: página inicial, recarga e console com token. */
async function fakeKubejs(): Promise<FakeKubejs> {
  const reloads: string[] = [];
  const server = createServer((request, response) => {
    const auth = request.headers.authorization ?? '';
    if (request.method === 'GET' && request.url === '/') {
      response.end('KubeJS Local Web Server [Minecraft]\n');
      return;
    }
    if (auth !== `Bearer ${TOKEN}`) {
      response.statusCode = 401;
      response.end('Missing Authorization header');
      return;
    }
    if (request.method === 'POST' && request.url === '/api/reload/server') {
      reloads.push(auth);
      response.end('{}');
      return;
    }
    if (request.method === 'GET' && request.url?.startsWith('/api/console/')) {
      response.setHeader('content-type', 'application/json');
      response.end('[]');
      return;
    }
    response.statusCode = 404;
    response.end();
  });
  await new Promise<void>((resolve) => {
    server.listen(0, '127.0.0.1', resolve);
  });
  return { server, port: (server.address() as AddressInfo).port, reloads };
}

describe('Editor de scripts (C-07; T26)', function () {
  let kubejs: FakeKubejs | null = null;

  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do editor de scripts pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack('2001.6.5-build.26');
    // O jar do Minecraft "instalado" pelo launcher, com os IDs de cobre.
    mkdirSync(join(MINECRAFT_JAR, '..'), { recursive: true });
    writeFileSync(
      MINECRAFT_JAR,
      storedZip({
        'assets/minecraft/models/item/copper_ingot.json': '{}',
        'assets/minecraft/models/item/copper_block.json': '{}',
        'assets/minecraft/blockstates/copper_ore.json': '{}',
        'assets/minecraft/models/item/stone.json': '{}',
      }),
    );
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(REGISTRY_FILE, { force: true });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    if (kubejs) {
      kubejs.server.close();
    }
    try {
      rmSync(join(instanceGameDir(), '..'), { recursive: true, force: true });
    } catch {
      // Sem registro: o teste foi pulado antes de abrir o pack.
    }
    rmSync(MINECRAFT_JAR, { force: true });
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('CA-T26-01: abre o script com a versão, os trechos prontos e o autocompletar de IDs', async () => {
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

    await goToConfigs();
    await openFile('receitas.js');
    await $('.tag*=KubeJS 6 · Forge 1.20.1').waitForDisplayed({ timeout: 30_000 });
    await expect($('.editor__foot')).toHaveText(expect.stringContaining('JavaScript · KubeJS 6'));

    // Sem jogo aberto: Recarregar no jogo fica indisponível com o motivo.
    const reload = $('button=Recarregar no jogo');
    await expect(reload).toHaveAttribute('aria-disabled', 'true');
    await expect(reload).toHaveAttribute('aria-describedby');

    // Trechos prontos da versão (só os de server_scripts).
    await $('button=Trechos prontos').click();
    const menu = $('[role="menu"]');
    await menu.waitForDisplayed();
    await expect(menu).toHaveText(expect.stringContaining('Receita com forma'));
    await expect(menu).toHaveText(expect.stringContaining('Tag de item'));
    expect(await menu.getText()).not.toContain('Dica de item');
    await screenshot('60-scripts-trechos');
    expect(await seriousViolationSummaries()).toEqual([]);
    await browser.keys('Escape');
    await menu.waitForDisplayed({ reverse: true });

    // Autocompletar de IDs com os do jar do Minecraft.
    await $('.cm-content').click();
    await browser.keys(['Control', 'End']);
    await browser.keys('Enter');
    await typeSlowly("'minecraft:copper_");
    const list = $('.cm-tooltip-autocomplete');
    await list.waitForDisplayed({ timeout: 15_000 });
    await expect(list).toHaveText(expect.stringContaining('minecraft:copper_ingot'));
    expect(await list.getText()).not.toContain('minecraft:stone');
    await screenshot('61-scripts-autocompletar');
    await browser.keys('Escape');
    await $('button=Desfazer as alterações').click();
    await $('[role="alertdialog"]').$('button=Desfazer as alterações').click();
    await expect($('.editor__foot')).toHaveText(expect.stringContaining('Sem alterações'));
  });

  it('CA-T26-04: com o jogo aberto, o KubeJS 6 mostra o comando para copiar', async () => {
    await dismissToasts();
    await $('header.packhead .testbtn__main').click();
    await waitHeading('Jogo aberto');
    await $('main').$('button=Recarregar scripts').click();
    await waitToast('No KubeJS 6, recarregar precisa de um comando no jogo.');
    await waitToast('Copiar /kubejs reload server_scripts');
    await screenshot('62-scripts-tela-do-teste');

    // No editor, na instância de teste.
    await goToConfigs();
    await $('select[aria-label="Origem dos arquivos"]').selectByVisibleText(
      'Mostrando: instância de teste',
    );
    await openFile('receitas.js');
    const reload = $('button=Recarregar no jogo');
    await browser.waitUntil(async () => (await reload.getAttribute('aria-disabled')) === null, {
      timeout: 30_000,
      timeoutMsg: 'Recarregar no jogo não ficou disponível com o jogo aberto',
    });
    await reload.click();
    await $('button=Copiar /kubejs reload server_scripts').waitForDisplayed({ timeout: 30_000 });
    await screenshot('63-scripts-comando');
  });

  it('CA-T26-02: o erro do log real aparece na linha 16 e clicar leva à linha', async () => {
    const game = instanceGameDir();
    mkdirSync(join(game, 'logs', 'kubejs'), { recursive: true });
    writeFileSync(
      join(game, 'logs', 'kubejs', 'server.log'),
      readFileSync(join(REAL_LOGS, 'logs', 'kubejs', 'server.log')),
    );
    // Abre de novo para reler os logs.
    await $('nav[aria-label="Seções do pack"]').$('a*=Mods').click();
    await waitHeading('Mods');
    await goToConfigs();
    await openFile('receitas.js');
    const panel = $('section[aria-labelledby]*=Erros dos scripts');
    await panel.waitForDisplayed({ timeout: 30_000 });
    const location = panel.$('button=receitas.js, linha 16');
    await location.waitForDisplayed({ timeout: 30_000 });
    await expect(panel).toHaveText(expect.stringContaining("ItemStack 'result' can't be empty!"));
    await expect(panel).toHaveText(expect.stringContaining('receitas.js#16'));
    await location.click();
    await browser.waitUntil(async () => (await footer()).includes('linha 16, coluna 1'), {
      timeout: 15_000,
      timeoutMsg: 'o cursor não foi para a linha 16',
    });
    await screenshot('64-scripts-erros');
    await dismissToasts();
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('CA-T26-03: com o KubeJS 7, recarrega pelo servidor web com o token; o token não aparece', async () => {
    kubejs = await fakeKubejs();
    const game = instanceGameDir();
    mkdirSync(join(game, 'kubejs', 'config'), { recursive: true });
    writeFileSync(
      join(game, 'kubejs', 'config', 'web_server.json'),
      JSON.stringify({ enabled: true, port: kubejs.port, public_address: '', auth: TOKEN }),
    );
    // O pack passa a ter o KubeJS 7.
    writeFileSync(join(PACK, 'mods', 'kubejs-forge.jar'), kubejsJar('2101.7.2-build.377'));
    await $('nav[aria-label="Seções do pack"]').$('a*=Mods').click();
    await waitHeading('Mods');
    await goToConfigs();
    await openFile('receitas.js');
    await $('.tag*=KubeJS 7 · Forge 1.20.1').waitForDisplayed({ timeout: 30_000 });
    await $('span*=ao vivo, do jogo aberto').waitForDisplayed({ timeout: 30_000 });

    await dismissToasts();
    await $('button=Recarregar no jogo').click();
    await waitToast('Scripts recarregados no jogo');
    expect(kubejs.reloads).toEqual([`Bearer ${TOKEN}`]);
    await screenshot('65-scripts-kubejs-7');

    // Varredura: tela, registros do Warden e pasta do pack.
    const html = await browser.execute(() => document.documentElement.outerHTML);
    expect(html).not.toContain(TOKEN);
    expect(allText(join(DATA_ROOT, 'data', 'logs'))).not.toContain(TOKEN);
    expect(allText(join(DATA_ROOT, 'config'))).not.toContain(TOKEN);
    expect(allText(PACK)).not.toContain(TOKEN);
  });

  it('no pack com o jogo aberto: Salvar e recarregar grava no pack, copia para a instância e recarrega', async () => {
    if (!kubejs) throw new Error('o teste do KubeJS 7 não rodou');
    const game = instanceGameDir();
    await dismissToasts();
    await $('select[aria-label="Origem dos arquivos"]').selectByVisibleText(
      'Mostrando: arquivos do pack',
    );
    await openFile('receitas.js');
    await waitAlert('Salvar grava no pack e copia o script');
    await dismissToasts();
    await $('.cm-content').click();
    await browser.keys(['Control', 'End']);
    await browser.keys('Enter');
    await typeSlowly('// salvo pelo pack');
    const saveAndReload = $('button=Salvar e recarregar');
    await saveAndReload.waitForDisplayed({ timeout: 15_000 });
    await dismissToasts();
    await saveAndReload.click();
    await waitToast('Scripts recarregados no jogo');
    expect(kubejs.reloads).toHaveLength(2);
    const packText = readFileSync(join(PACK, RECEITAS), 'utf8');
    expect(packText).toContain('// salvo pelo pack');
    expect(readFileSync(join(game, RECEITAS), 'utf8')).toBe(packText);
    await screenshot('66-scripts-salvar-no-pack');

    // Mudança feita direto na instância: o Warden salva no pack e pergunta antes de substituir.
    writeFileSync(join(game, RECEITAS), '// mexido na instância\n');
    await dismissToasts();
    await $('.cm-content').click();
    await browser.keys(['Control', 'End']);
    await browser.keys('Enter');
    await typeSlowly('// outro');
    await browser.keys(['Control', 's']);
    // Com "mostrar as diferenças antes de salvar" ligado (o padrão), confirma no diálogo.
    const confirm = $('[role="dialog"]').$('button=Salvar');
    await confirm.waitForDisplayed({ timeout: 15_000 });
    await confirm.click();
    const conflict = await waitAlert('O script na instância de teste foi mudado direto nela.');
    expect(readFileSync(join(PACK, RECEITAS), 'utf8')).toContain('// outro');
    expect(readFileSync(join(game, RECEITAS), 'utf8')).toBe('// mexido na instância\n');
    expect(kubejs.reloads).toHaveLength(2);
    await dismissToasts();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('67-scripts-instancia-mexida');
    await conflict.$('button=Substituir na instância').click();
    await browser.waitUntil(
      () =>
        readFileSync(join(game, RECEITAS), 'utf8') === readFileSync(join(PACK, RECEITAS), 'utf8'),
      { timeout: 30_000, timeoutMsg: 'a instância não recebeu o script do pack' },
    );
    await waitAlert('Salvo no pack e copiado para a instância de teste.');
  });

  it('Parar jogo', async () => {
    await dismissToasts();
    await $('header.packhead .testbtn__main').click();
    await waitHeading('Jogo aberto');
    await $('main').$('button=Parar jogo').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await dialog.$('button=Parar jogo').click();
    await waitHeading('Encerrado por você', 30_000);
  });
});
