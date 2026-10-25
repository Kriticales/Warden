/**
 * Diagnóstico sem IA no app real (D-03; SPEC T13 e T14), com o packwiz de verdade, o Modrinth
 * simulado (servidor de fixtures) e o jogo simulado da `warden-launcher` (`WardenFakeGame`,
 * rodado pelo Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`).
 *
 * - Problemas verifica sozinha um pack nunca verificado: o Sodium está como "Cliente e
 *   servidor", mas o Modrinth diz que só funciona no cliente (aviso, com o contador no menu e a
 *   marca na lista de Mods); "Mudar lado para Só cliente" corrige pelo "Alterar lado" de sempre
 *   e a verificação seguinte não acha mais nada (correção "Mudar lado" de ponta a ponta).
 * - Pack quebrado: um mod que exige o Sodium, que não está no pack. O Testar passa por
 *   "Verificar o pack", copia o pack e para na "Verificação final" com o diálogo dos erros;
 *   "Testar mesmo assim" abre o jogo simulado, que trava, e o resultado é "Travou" com "Por que
 *   travou" (CA-T13-06): a causa e o trecho do crash report com o arquivo e a linha.
 * - "Adicionar sodium" no diálogo acha o projeto no Modrinth e abre o diálogo de dependências
 *   de sempre; confirmar grava o `.pw.toml` do Sodium (correção "Adicionar dependência" de ponta
 *   a ponta).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { storedZip } from './mock-server/jars';
import { dismissToasts } from './toasts';
import { openOrImport } from './open-or-import';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-diagnostico');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** O Sodium como o packwiz grava (versão gravada `SMxNOGZ6`), com o lado dado. */
function sodiumMetafile(side: string): string {
  return [
    'name = "Sodium"',
    'filename = "sodium-fabric-0.8.13+mc1.21.1.jar"',
    `side = "${side}"`,
    '',
    '[download]',
    'url = "https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar"',
    'hash-format = "sha512"',
    'hash = "8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e54e8be06d5ea5a9c268beb0e02073209d1f831a02dede851588b972367680853"',
    '',
    '[update]',
    '[update.modrinth]',
    'mod-id = "AANobbMI"',
    'version = "SMxNOGZ6"',
    '',
  ].join('\n');
}

/** Um mod Fabric que exige o Sodium. */
const MEU_MOD = storedZip({
  'fabric.mod.json': JSON.stringify({
    schemaVersion: 1,
    id: 'meumod',
    version: '1.0.0',
    environment: '*',
    depends: { sodium: '*' },
  }),
});

/** Um pack Fabric 1.21.1 só com os arquivos dados no índice e o modo do jogo simulado. */
function writePack(files: Record<string, string | Buffer>, fakeGame: string): void {
  rmSync(join(PACK, 'mods'), { recursive: true, force: true });
  mkdirSync(PACK, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, content]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, content);
      const metafile = path.endsWith('.pw.toml') ? 'metafile = true\n' : '';
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(content)}"\n${metafile}`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale Quebrado"',
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
      'fabric = "0.16.10"',
      'minecraft = "1.21.1"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(PACK, '.warden-fake-game'), fakeGame);
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.pause(400);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

function testButton() {
  return $('header.packhead .testbtn__main');
}

/** Um achado pelo título (`article` com `aria-labelledby`: a linha em Problemas, o cartão no
 * resultado do teste). */
function finding(title: string) {
  return $(`article*=${title}`);
}

describe('Diagnóstico: Problemas e Testar (D-03; T13 e T14)', function () {
  before(async function () {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack({ 'mods/sodium.pw.toml': sodiumMetafile('both') }, 'crash');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    rmSync(REGISTRY_FILE, { force: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // A instância de teste deste pack sai junto: os outros E2E do Testar usam a única que houver.
    if (existsSync(REGISTRY_FILE)) {
      const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
        packs?: { id: string; path: string }[];
      };
      for (const pack of registry.packs ?? []) {
        if (pack.path === PACK) {
          rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
        }
      }
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('Problemas verifica sozinha: aviso de lado errado, contador no menu e marca em Mods', async () => {
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

    await sectionLink('Problemas').click();
    await waitHeading('Problemas');
    const side = finding('Sodium está no lado errado');
    await side.waitForDisplayed({ timeout: 60_000 });
    expect((await side.getText()).toLowerCase()).toContain('aviso');
    // A evidência fica à vista, em mono (esboço aprovado do bug-01).
    await expect(side).toHaveText(expect.stringContaining('server_side'));
    await expect(side.$('a*=Perguntar ao Sculk')).toBeDisplayed();
    // Contador: 1 aviso.
    await expect(sectionLink('Problemas')).toHaveText(expect.stringContaining('1'));
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d03-01-problemas-lado');

    // A marca na lista de Mods leva de volta a Problemas.
    await sectionLink('Mods').click();
    await waitHeading('Mods');
    const mark = $('tr[data-path="mods/sodium.pw.toml"]').$('a.badge*=Aviso');
    await mark.waitForDisplayed({ timeout: 30_000 });
    await screenshot('d03-02-marca-em-mods');
    await mark.click();
    await waitHeading('Problemas');

    await finding('Sodium está no lado errado').$('button=Mudar lado para Só cliente').click();
    await $('h2=Nenhum problema encontrado').waitForDisplayed({ timeout: 60_000 });
    expect(readFileSync(join(PACK, 'mods', 'sodium.pw.toml'), 'utf8')).toContain('side = "client"');
    await screenshot('d03-03-lado-corrigido');
  });

  it('pack quebrado: o Testar para na Verificação final com o diálogo dos erros', async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): Testar do diagnóstico pulado');
      this.skip();
    }
    writePack({ 'mods/meu-mod.jar': MEU_MOD }, 'crash');
    await sectionLink('Mods').click();
    await waitHeading('Mods');
    await testButton().click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed({ timeout: 90_000 });
    await expect(dialog).toHaveText(expect.stringContaining('1 erro pode impedir o jogo de abrir'));
    await expect(dialog).toHaveText(
      expect.stringContaining('meu-mod.jar precisa de sodium, que não está no pack'),
    );
    await expect(dialog.$('button=Adicionar sodium')).toBeDisplayed();
    await expect(dialog.$('button=Cancelar e ver Problemas')).toBeDisplayed();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d03-04-dialogo-erros');
  });

  it('CA-T13-06: "Testar mesmo assim" abre o jogo, que trava, e o resultado mostra "Por que travou"', async function () {
    if (!HAS_JAVA) this.skip();
    await $('[role="dialog"]').$('button=Testar mesmo assim').click();
    await waitHeading('O jogo travou', 90_000);
    const why = $('section[aria-labelledby="why-crashed-title"]');
    await expect(why).toHaveText(expect.stringContaining('Por que travou'));
    const cause = why.$('article.issue*=O jogo travou: Initializing game');
    await cause.waitForDisplayed({ timeout: 30_000 });
    await expect(cause).toHaveText(
      expect.stringContaining('crash-reports/crash-teste-client.txt, linha 2'),
    );
    await expect(why.$('button=Copiar relatório')).toBeDisplayed();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('d03-05-por-que-travou');
  });

  it('"Adicionar sodium" no diálogo do Testar grava a dependência pelo diálogo de sempre', async function () {
    if (!HAS_JAVA) this.skip();
    await $('main').$('button=Testar de novo').click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed({ timeout: 90_000 });
    await dialog.$('button=Adicionar sodium').click();
    const deps = $('[role="dialog"]*=Sodium');
    await deps.waitForDisplayed({ timeout: 30_000 });
    const add = $('[role="dialog"]').$('button=Adicionar 1 item');
    await add.waitForDisplayed({ timeout: 30_000 });
    await screenshot('d03-06-dependencias');
    await add.waitForClickable({ timeout: 30_000 });
    await add.click();
    await browser.waitUntil(() => existsSync(join(PACK, 'mods', 'sodium.pw.toml')), {
      timeout: 30_000,
      timeoutMsg: 'o .pw.toml do Sodium não foi gravado',
    });
    await screenshot('d03-06b-depois-de-adicionar');
    expect(readFileSync(join(PACK, 'mods', 'sodium.pw.toml'), 'utf8')).toContain('AANobbMI');
    await $('button=Testar de novo').waitForDisplayed();

    // Em Problemas, a dependência não falta mais (o jar do Sodium ainda não foi baixado: a
    // verificação completa fica para o Testar, e a seção diz isso).
    await sectionLink('Problemas').click();
    await waitHeading('Problemas');
    await $('.alert*=A verificação completa roda ao clicar em Testar').waitForDisplayed({
      timeout: 60_000,
    });
    await expect(finding('precisa de sodium')).not.toBeExisting();
    await screenshot('d03-07-dependencia-adicionada');
  });
});
