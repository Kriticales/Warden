/**
 * Fluxo completo de ponta a ponta no app real (A-03; SPEC T03, T08, T09, T12, T13, T15, T17, T18
 * e T19), num só pack e na ordem em que o dono trabalha: criar o pack → adicionar pela busca
 * combinada → configurar → **Testar** → trazer o que mudou no teste → **Salvar versão** final →
 * **Publicar versão** → instalar como um jogador, pelo **link do pack** → **Exportar** o .zip.
 *
 * Tudo é o de verdade (packwiz, git, packwiz-installer-bootstrap e packwiz-installer dos jars
 * fixados) menos o que sairia do computador: o catálogo de versões, o Modrinth, a CurseForge e o
 * GitHub são os simulados do servidor de fixtures, e o jogo é o simulado da `warden-launcher`
 * (fica aberto até **Parar jogo**). Os arquivos dos mods vêm do mesmo servidor, pelos downloads
 * locais (`mock-server/local-downloads.ts`), para o Testar e o instalador do jogador não irem à
 * internet.
 *
 * Cada passo confere o que aparece na tela e o que ficou no disco:
 *
 * 1. Configurações: a chave da CurseForge e o token do GitHub ficam guardados (••••final).
 * 2. Criar pack Fabric 1.21.1 com os mods iniciais (spark, Crash Assistant e a Fabric API que o
 *    spark traz): o pack abre em Mods com os três, e a config do Crash Assistant existe.
 * 3. Adicionar: a busca combinada mostra o Sodium uma vez, "Modrinth e CurseForge", e o mod só da
 *    CurseForge; o Sodium entra pelo Modrinth e aparece em Mods.
 * 4. Configs: a config do Crash Assistant editada no texto e salva com a diferença; o índice muda.
 * 5. Testar: o jogo simulado abre; o "jogo" muda uma chave da config na instância; Parar jogo.
 * 6. O que mudou durante o teste: só essa chave; trazer grava só ela no pack, por cima da edição.
 * 7. Salvar versão 1.0.0 como versão final; o cabeçalho fica sem alterações.
 * 8. Publicar versão: cria o repositório, avisa que a 1.0.0 não foi testada (o teste foi parado
 *    e veio antes de trazer a mudança) e publica; o link do pack serve o `pack.toml` da versão.
 * 9. Testar como o jogador recebe: o packwiz-installer-bootstrap instala pelo link do pack numa
 *    instância limpa, e cada arquivo do índice chega igual ao do pack (os mods, iguais ao arquivo
 *    que o metafile aponta, conferidos pelo hash); o resultado diz que veio do link da 1.0.0.
 * 10. Exportar .zip: as entradas e os bytes são os do pack; Meus packs mostra a versão 1.0.0.
 *
 * Sem Java ou sem os jars do instalador (`cargo xtask installer`), avisa e pula: a CI tem os
 * dois. Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { CHAVE_BOA } from './mock-server/curseforge';
import { GITHUB_LOGIN, GITHUB_TOKEN, type GithubState } from './mock-server/github';
import {
  closeDialog,
  dialogNamed,
  expectedFiles,
  exportAndWait,
  openSection,
  saveFinal,
  stopGame,
  waitToast,
} from './steps';
import { keyItem, openKeysPage, saveKey } from './keys';
import { dismissToasts } from './toasts';
import { zipEntries, zipEntry } from './zip';

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
const NAME = 'Vale do Fluxo';
const PACK = join(DATA_ROOT, 'packs', 'vale-do-fluxo');
const REPO = `${GITHUB_LOGIN}/vale-do-fluxo`;
const PACK_LINK = `${MOCK_URL}/github/raw/${REPO}/main/pack.toml`;
const CONFIG = 'config/crash_assistant/config.toml';
const ZIP = join(DATA_ROOT, 'exportados-fluxo', 'vale-do-fluxo.zip');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const SECRETS = join(DATA_ROOT, 'cofre-de-teste');
const INSTANCES = join(DATA_ROOT, 'data', 'instances');
const TMP = join(DATA_ROOT, 'data', 'cache', 'tmp');
const METADATA_DB = join(DATA_ROOT, 'data', 'cache', 'metadata.sqlite');

/** A linha que o dono acrescenta à config em Configs. */
const OWNER_LINE = '# Ajuste do dono: sem aviso de pirataria nos testes.';

/** As instâncias de teste que já existiam antes deste spec (de outros specs). */
let instancesBefore = new Set<string>();
/** A config como ficou depois de editada em Configs. */
let editedConfig = '';

function instanceIds(): string[] {
  return existsSync(INSTANCES) ? readdirSync(INSTANCES) : [];
}

function read(path: string): string {
  return readFileSync(join(PACK, path), 'utf8');
}

function hash(algorithm: string, data: Buffer): string {
  return createHash(algorithm).update(data).digest('hex');
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function mock(path: string): Promise<void> {
  const response = await fetch(`${MOCK_URL}${path}`, { method: 'POST' });
  expect(response.ok).toBe(true);
}

async function github(): Promise<GithubState> {
  const response = await fetch(`${MOCK_URL}/__github`);
  return (await response.json()) as GithubState;
}

/**
 * Apaga o cache de metadados do app com o app fechado (no Windows o arquivo aberto não sai). Quem
 * chama abre o app de novo, se precisar.
 */
async function clearMetadataCache(): Promise<void> {
  await browser.deleteSession();
  for (const suffix of ['', '-wal', '-shm']) {
    rmSync(`${METADATA_DB}${suffix}`, { force: true, maxRetries: 5, retryDelay: 200 });
  }
}

/** A pasta do jogo da instância de teste deste pack (a que tem a config do Crash Assistant). */
function testGameDir(): string {
  for (const id of instanceIds()) {
    if (instancesBefore.has(id)) continue;
    const game = join(INSTANCES, id, 'minecraft');
    if (existsSync(join(game, CONFIG))) return game;
  }
  throw new Error('instância de teste do fluxo completo ausente');
}

/** A instância limpa do teste como o jogador em andamento (`cache/tmp/player-test-*`). */
function playerGameDir(): string | null {
  if (!existsSync(TMP)) return null;
  const found = readdirSync(TMP).find((name) => name.startsWith('player-test-'));
  return found ? join(TMP, found, 'minecraft') : null;
}

/** As entradas do índice do pack: caminho e se é metafile (`.pw.toml`). */
function indexEntries(): { file: string; metafile: boolean }[] {
  return read('index.toml')
    .split('[[files]]')
    .slice(1)
    .map((block) => ({
      file: /^file = "([^"]+)"/m.exec(block)?.[1] ?? '',
      metafile: /^metafile = true/m.test(block),
    }));
}

/** O texto de um campo do metafile (`filename = "…"`). */
function metafileField(text: string, field: string): string {
  return new RegExp(`^${field} = "([^"]+)"`, 'm').exec(text)?.[1] ?? '';
}

/** Espera o jogo simulado abrir (o console mostra o processo filho vivo). */
async function waitGameOpen(): Promise<void> {
  await $('h1*=Jogo aberto').waitForDisplayed({ timeout: 120_000 });
  const console = $('section[aria-label="Console do jogo"]');
  await browser.waitUntil(async () => (await console.getText()).includes('pai vivo'), {
    timeout: 30_000,
    timeoutMsg: 'o console não mostrou o jogo simulado',
  });
}

/** A linha de resultado da busca com este nome. */
function resultRow(name: string) {
  return $(
    `.//div[@role="row"][.//button[contains(@class,"tname__n")][normalize-space()="${name}"]]`,
  );
}

describe('Fluxo completo: criar, adicionar, configurar, testar, salvar, publicar, instalar e exportar (A-03)', function () {
  before(async function () {
    if (!READY) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java ou sem os jars do instalador (cargo xtask installer): E2E pulado');
      this.skip();
    }
    instancesBefore = new Set(instanceIds());
    rmSync(PACK, { recursive: true, force: true });
    rmSync(join(ZIP, '..'), { recursive: true, force: true });
    mkdirSync(join(ZIP, '..'), { recursive: true });
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(PICK_FILE, '');
    await mock('/__github/reset');
    await mock('/__downloads-locais/ligar');
    // As versões do Modrinth não vencem no cache do app (`metadata.sqlite`): as que outros specs
    // guardaram trariam os links da internet.
    await clearMetadataCache();
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(async function () {
    if (!READY) return;
    await mock('/__downloads-locais/desligar');
    await mock('/__github/reset');
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(join(SECRETS, 'curseforge-api-key'), { force: true });
    rmSync(join(SECRETS, 'github-token'), { force: true });
    // E as deste spec, com os links locais, não vão para os specs seguintes.
    await clearMetadataCache();
    // Os outros specs contam com as instâncias que já existiam (o do Testar, com uma só).
    for (const id of instanceIds()) {
      if (!instancesBefore.has(id)) rmSync(join(INSTANCES, id), { recursive: true, force: true });
    }
    rmSync(PACK, { recursive: true, force: true });
  });

  it('1. Configurações: guarda a chave da CurseForge e o token do GitHub', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    await openKeysPage();
    for (const [kind, value] of [
      ['curseforge', CHAVE_BOA],
      ['github', GITHUB_TOKEN],
    ] as const) {
      await saveKey(kind, value);
      await expect(keyItem(kind)).toHaveText(expect.stringContaining(`••••${value.slice(-4)}`));
    }
    const html = await browser.execute(() => document.documentElement.outerHTML);
    expect(html.includes(CHAVE_BOA)).toBe(false);
    expect(html.includes(GITHUB_TOKEN)).toBe(false);
    await dismissToasts();
    await $('header.topbar').$('a*=Meus packs').click();
    await $('h1=Meus packs').waitForDisplayed();
  });

  it('2. Criar pack: Fabric 1.21.1 com os mods iniciais, aberto em Mods', async () => {
    const wizard = $('[role="dialog"]');
    const next = async (label = 'Próximo') => {
      const button = wizard.$('.dialog__foot').$(`button*=${label}`);
      await button.waitForEnabled();
      await button.click();
    };
    await $('button=Criar pack').click();
    await wizard.$('h2=Criar pack').waitForDisplayed();
    const nameId = await $('label=Nome do pack').getAttribute('for');
    await $(`[id="${nameId ?? ''}"]`).setValue(NAME);
    const folder = $('input.input--mono');
    await browser.waitUntil(async () => (await folder.getValue()).endsWith('vale-do-fluxo'), {
      timeoutMsg: 'a pasta não acompanhou o nome do pack',
    });
    await next();

    await wizard.$('input[type="search"]').waitForDisplayed();
    await wizard.$('input[type="search"]').setValue('1.21.1');
    await $('label.check=1.21.1').click();
    await next();

    await $('span.choice__title=Fabric').click();
    await browser.waitUntil(() => $('.choice--split*=Fabric').$('input').isSelected());
    await next();

    // Os dois mods iniciais vêm marcados (D16); o spark traz a Fabric API junto.
    await $('legend*=Recomendados para Minecraft 1.21.1 com Fabric').waitForDisplayed({
      timeout: 30_000,
    });
    const spark = $('input[aria-label="Adicionar spark"]');
    const crash = $('input[aria-label="Adicionar Crash Assistant"]');
    await browser.waitUntil(async () => (await spark.isSelected()) && (await crash.isSelected()), {
      timeout: 30_000,
      timeoutMsg: 'os mods iniciais não vieram marcados',
    });
    await next();

    await $('.packcard__meta*=Minecraft 1.21.1 · Fabric').waitForDisplayed();
    await screenshot('a03-01-criar-resumo');
    expect(await seriousViolationSummaries()).toEqual([]);
    await next('Criar pack');

    await $(`h1=${NAME}`).waitForDisplayed({ timeout: 120_000 });
    await $('h1=Mods').waitForDisplayed();
    for (const name of ['spark', 'Crash Assistant', 'Fabric API']) {
      await $(`tr.modrow*=${name}`).waitForDisplayed({ timeout: 30_000 });
    }
    expect(read('pack.toml')).toContain('minecraft = "1.21.1"');
    expect(existsSync(join(PACK, '.git'))).toBe(true);
    expect(existsSync(join(PACK, CONFIG))).toBe(true);
    // Os arquivos dos mods vêm do servidor de fixtures (downloads locais), não da internet.
    expect(read('mods/crash-assistant.pw.toml')).toContain(`url = "${MOCK_URL}/jogador/`);
    await screenshot('a03-02-pack-criado');
  });

  it('3. Adicionar: a busca combinada mostra o Sodium uma vez, nas duas fontes; ele entra pelo Modrinth', async () => {
    await dismissToasts();
    await $('a=Adicionar').click();
    await $('h1=Adicionar ao pack').waitForDisplayed();
    await $('input[type="search"]').setValue('sodium');
    await $('.trow*=Mod Só da CurseForge').waitForDisplayed({ timeout: 30_000 });
    // A fonte é a marca (quadrado e losango), com o nome para o leitor de tela.
    const rowText = async (name: string) =>
      String(await resultRow(name).getProperty('textContent'));
    expect(await rowText('Sodium')).toContain('Modrinth e CurseForge');
    expect(await rowText('Mod Só da CurseForge')).not.toContain('Modrinth');
    expect(await $('body').getText()).not.toContain('Mostrando só o Modrinth.');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-03-busca-combinada');

    await $('button.tname__n=Sodium').click();
    const preview = $('aside.disc__preview');
    await preview.waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(
      async () => {
        const text = String(await preview.getProperty('textContent'));
        return text.includes('o SHA-1 confere') && text.includes('Modrinth (recomendada)');
      },
      { timeout: 30_000, timeoutMsg: 'a pré-visualização não conferiu as duas fontes' },
    );
    await dismissToasts();
    await preview.$('button=Adicionar ao pack').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await browser.waitUntil(
      async () => String(await dialog.getProperty('textContent')).includes('entrar no pack'),
      { timeout: 30_000, timeoutMsg: 'o plano não apareceu no diálogo' },
    );
    await dialog.$('button=Adicionar 1 item').click();
    await waitToast('1 item adicionado ao pack');
    await dialog.waitForExist({ reverse: true });
    await resultRow('Sodium').$('.tag*=No pack').waitForDisplayed({ timeout: 30_000 });

    const sodium = read('mods/sodium.pw.toml');
    expect(sodium).toContain('[update.modrinth]');
    expect(sodium).toContain('mod-id = "AANobbMI"');
    expect(read('index.toml')).toContain('mods/sodium.pw.toml');

    await $('a[aria-label^="Voltar para Mods"]').click();
    await $('h1=Mods').waitForDisplayed();
    await $('tr.modrow*=Sodium').waitForDisplayed({ timeout: 30_000 });
  });

  it('4. Configs: edita a config do Crash Assistant e salva vendo a diferença', async () => {
    await openSection('Configs', 'Configs');
    const tree = $('[role="tree"]');
    await tree.waitForDisplayed();
    // A árvore abre as pastas do caminho até o arquivo.
    await tree.$('[role="treeitem"]*=crash_assistant').click();
    await tree.$('[role="treeitem"]*=config.toml').click();
    const editor = $('.cm-content');
    await editor.waitForDisplayed({ timeout: 30_000 });
    await expect(editor).toHaveText(expect.stringContaining('[piracy]'));
    await expect($('button=Salvar')).toBeDisabled();
    const before = read(CONFIG);

    await dismissToasts();
    await editor.click();
    await browser.keys(['Control', 'End']);
    // O WebKitWebDriver perde os espaços teclados no CodeMirror (o caractere e a tecla Espaço);
    // a linha entra como texto digitado (`insertText`, o mesmo evento do teclado), no cursor.
    await browser.execute(
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- único jeito de digitar espaços no WebKitGTK
      (text) => document.execCommand('insertText', false, text),
      OWNER_LINE,
    );
    await expect($('.editor__foot')).toHaveText(expect.stringContaining('ainda não salva'));
    await $('button=Salvar').click();
    const dialog = $('[role="dialog"]*=Salvar config.toml?');
    await dialog.waitForDisplayed();
    // A diferença marca cada palavra num elemento: o `textContent` tem os espaços.
    await expect(dialog).toHaveElementProperty('textContent', expect.stringContaining(OWNER_LINE));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-04-config-diferenca');
    await dialog.$('button=Salvar').click();
    await waitToast('config.toml salvo.');

    editedConfig = read(CONFIG);
    expect(editedConfig).toBe(`${before}${OWNER_LINE}`);
    const configHash = hash('sha256', Buffer.from(editedConfig, 'utf8'));
    expect(read('index.toml')).toContain(`hash = "${configHash}"`);
  });

  it('5. Testar: o jogo simulado abre e muda uma chave da config na instância; Parar jogo', async () => {
    await dismissToasts();
    await $('header.packhead .testbtn__main').click();
    await waitGameOpen();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-05-jogo-aberto');
    // A instância recebeu a config editada; o "jogo" liga o aviso de pirataria.
    const file = join(testGameDir(), CONFIG);
    const synced = readFileSync(file, 'utf8');
    expect(synced).toBe(editedConfig);
    writeFileSync(file, synced.replace('\tenabled = false', '\tenabled = true'));
    await stopGame();
  });

  it('6. O que mudou durante o teste: só a chave do jogo; trazer grava só ela no pack', async () => {
    const block = $('section.capture');
    await block.waitForDisplayed({ timeout: 30_000 });
    await expect(block).toHaveText(expect.stringContaining('Durante o teste, 1 arquivo mudou'));
    await expect(block).toHaveText(expect.stringContaining('piracy.enabled'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-06-o-que-mudou');
    await dismissToasts();
    const bring = block.$('button=Trazer 1 selecionado para o pack');
    await bring.waitForClickable();
    await bring.click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('1 mudança trazida para o pack'));
    await dialog.$('button=Continuar editando').click();
    await browser.waitUntil(
      async () => (await $('main').getText()).includes('1 mudança foi trazida para o pack'),
      { timeout: 15_000, timeoutMsg: 'a tela não disse que a mudança foi trazida' },
    );
    // A linha do dono continua; só a chave do jogo mudou.
    expect(read(CONFIG)).toBe(editedConfig.replace('\tenabled = false', '\tenabled = true'));
  });

  it('7. Salvar versão: a 1.0.0 fica salva como versão final', async () => {
    await saveFinal('1.0.0');
    await screenshot('a03-07-versao-salva');
    expect(read('pack.toml')).toContain('version = "1.0.0"');
    expect(execFileSync('git', ['-C', PACK, 'tag', '-l'], { encoding: 'utf8' })).toContain('1.0.0');
  });

  it('8. Publicar versão: cria o repositório, avisa que não foi testada e publica', async () => {
    await $('[role="dialog"]').$('button=Publicar versão 1.0.0').click();
    const setup = await dialogNamed('Primeira publicação deste pack', 'Quem pode ver');
    await expect(setup.$('input.input--mono')).toHaveValue('vale-do-fluxo');
    await setup.$('button=Criar repositório e continuar').click();

    const plan = await dialogNamed(
      'Publicar a versão 1.0.0 para os jogadores',
      'A versão 1.0.0 não foi testada.',
      120_000,
    );
    await expect(plan).toHaveText(expect.stringContaining(`Vai para o GitHub (${REPO})`));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-08-publicar');
    await plan.$('button=Publicar mesmo assim').click();

    const result = await dialogNamed(
      'Versão 1.0.0 publicada',
      'Como os jogadores instalam',
      120_000,
    );
    await expect(result.$('.linkbox input')).toHaveValue(PACK_LINK);
    await expect(result).toHaveText(
      expect.stringContaining(`"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${PACK_LINK}`),
    );
    await screenshot('a03-09-publicado');
    // O link do pack serve o pack.toml da versão, com os mesmos bytes.
    const served = await fetch(PACK_LINK);
    expect(Buffer.from(await served.arrayBuffer())).toEqual(readFileSync(join(PACK, 'pack.toml')));
    const state = await github();
    expect(state.releases).toHaveLength(1);
    expect(state.releases[0]?.tag).toBe('v1.0.0');
    await closeDialog();
  });

  it('9. Testar como o jogador recebe: o bootstrap instala pelo link do pack e os arquivos chegam iguais', async () => {
    await fetch(`${MOCK_URL}/__reset`, { method: 'POST' });
    await dismissToasts();
    await $('header.packhead .testbtn__more').click();
    const item = $('[role="menuitem"]*=Testar como o jogador recebe');
    await item.waitForDisplayed();
    await item.click();
    await waitGameOpen();

    // A instância limpa tem cada arquivo do índice igual ao do pack; os mods, iguais ao arquivo
    // que o metafile aponta (conferido pelo hash dele).
    const game = playerGameDir();
    expect(game).not.toBeNull();
    const entries = indexEntries();
    expect(entries.map((entry) => entry.file).sort()).toEqual([
      CONFIG,
      'mods/crash-assistant.pw.toml',
      'mods/fabric-api.pw.toml',
      'mods/sodium.pw.toml',
      'mods/spark.pw.toml',
    ]);
    for (const entry of entries) {
      if (!entry.metafile) {
        expect(readFileSync(join(game ?? '', entry.file))).toEqual(
          readFileSync(join(PACK, entry.file)),
        );
        continue;
      }
      const meta = read(entry.file);
      const received = readFileSync(
        join(game ?? '', join(entry.file, '..'), metafileField(meta, 'filename')),
      );
      expect(hash(metafileField(meta, 'hash-format'), received)).toBe(metafileField(meta, 'hash'));
    }
    const seen = (await (await fetch(`${MOCK_URL}/__requests`)).json()) as string[];
    expect(seen).toContain(`GET /github/raw/${REPO}/main/pack.toml`);
    expect(seen.filter((line) => line.includes('/github/api/'))).toEqual([]);

    await stopGame();
    const result = $('section[aria-labelledby="player-result-title"]');
    await result.waitForDisplayed({ timeout: 30_000 });
    await expect(result).toHaveText(
      expect.stringContaining('Instalado do zero pelo link do pack (versão 1.0.0 publicada).'),
    );
    await expect(result).toHaveText(
      expect.stringContaining(`O jogador recebeu ${String(entries.length)} arquivos.`),
    );
    await expect(result).toHaveText(
      expect.stringContaining('O jogador recebeu o mesmo que o pack.'),
    );
    await expect(result).toHaveText(
      expect.stringContaining('O Crash Assistant (ferramenta do jogador) chegou ao jogador.'),
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-10-jogador');
    expect(playerGameDir()).toBeNull();
  });

  it('10. Exportar: o .zip tem os arquivos do pack, byte a byte; Meus packs mostra a 1.0.0', async () => {
    await openSection('Exportar', 'Exportar');
    await $('h2=3. O que vai no pack').waitForDisplayed({ timeout: 60_000 });
    await $('label*=Arquivo .zip do pack packwiz').click();
    await expect($('main')).toHaveText(
      expect.stringContaining('A seguir, você escolhe onde salvar o arquivo .zip.'),
    );
    writeFileSync(PICK_FILE, ZIP);
    await exportAndWait();
    await expect($('[role="dialog"]')).toHaveText(expect.stringContaining('Mostrar o arquivo'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('a03-11-exportado');
    await closeDialog();

    const files = expectedFiles(PACK);
    expect(zipEntries(ZIP)).toEqual(files);
    for (const file of files) {
      expect(zipEntry(ZIP, file)).toEqual(readFileSync(join(PACK, file)));
    }

    await dismissToasts();
    await $('header.packhead').$('a*=Meus packs').click();
    const row = $(`tr.packrow*=${NAME}`);
    await row.waitForDisplayed({ timeout: 30_000 });
    await expect(row).toHaveText(expect.stringContaining('1.0.0'));
    await expect(row).not.toHaveText(expect.stringContaining('Nunca testado'));
  });
});
