/**
 * Testar e console no app real (L-04; SPEC T13), com o jogo simulado da `warden-launcher`
 * (`WardenFakeGame`, rodado pelo Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`): o app não baixa
 * Java nem Minecraft, e todo o resto é o de verdade (um jogo por vez, cópia do pack para a
 * instância, processo, console, parar, sessão gravada). O modo do jogo simulado vem do arquivo
 * `.warden-fake-game` na pasta do pack.
 *
 * - CA-T13-08: com o jogo aberto o botão diz "Jogo aberto: ver teste" e, de Mods, leva à tela
 *   do teste; "Ver último teste" no ▾ abre a última sessão.
 * - CA-T13-02 (parte do app): "Parar jogo" encerra o jogo (com um processo filho) em até 5 s e
 *   o resultado é "Encerrado por você".
 * - CA-T13-06 (parte do app): um travamento termina com "O jogo travou" e "Por que travou".
 * - CA-T13-03 (parte do app): linhas com acentos aparecem certas no console.
 * - CA-T13-04: o mod que saiu do pack sai da instância; o mundo de teste fica.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const SAVE_FILE = env('WARDEN_E2E_SAVE_FILE');
const PACK = join(DATA_ROOT, 'fora', 'pack-testar');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Um pack vanilla 1.20.1 com os arquivos dados no índice e o modo do jogo simulado. */
function writePack(files: Record<string, string>, fakeGame: string): void {
  mkdirSync(PACK, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, text]) => {
      const file = join(PACK, path);
      mkdirSync(join(file, '..'), { recursive: true });
      writeFileSync(file, text);
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(text)}"\n`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(PACK, 'index.toml'), index);
  writeFileSync(
    join(PACK, 'pack.toml'),
    [
      'name = "Vale de Teste"',
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
  setFakeGame(fakeGame);
}

function setFakeGame(args: string): void {
  writeFileSync(join(PACK, '.warden-fake-game'), args);
}

/** A instância de teste do pack (a única em `instances/`). */
function instanceGameDir(): string {
  const instances = join(DATA_ROOT, 'data', 'instances');
  const [id] = readdirSync(instances);
  if (!id) throw new Error('instância de teste ausente');
  return join(instances, id, 'minecraft');
}

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

/** O botão principal do Testar no cabeçalho. */
function testButton() {
  return $('header.packhead .testbtn__main');
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

describe('Testar e console (L-04; T13)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do Testar pulado');
      this.skip();
    }
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack(
      { 'config/vale.toml': 'dificuldade = 2\n', 'mods/biblioteca.jar': 'jar de teste\n' },
      'filhos 1',
    );
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('abre o pack com o botão Testar e o menu ▾ em grupos', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    // O jar local e o arquivo do jogo simulado podem aparecer na higiene: abre sem limpar.
    const open = $('button=Abrir pack');
    const notNow = $('button=Agora não');
    await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
      timeout: 60_000,
      timeoutMsg: 'Abrir pack não ficou pronto',
    });
    await screenshot('39-abrir-pack');
    if (await notNow.isExisting()) await notNow.click();
    else await open.click();
    await waitHeading('Mods');
    await expect(testButton()).toHaveText('Testar');
    await screenshot('40-botao-testar');

    await $('header.packhead .testbtn__more').click();
    await $('[role="menuitem"]*=Ver último teste').waitForDisplayed();
    await expect($('.menu__label*=Perfil do teste')).toBeDisplayed();
    await expect($('.menu__label*=Instância de teste')).toBeDisplayed();
    await expect($('[role="menuitem"]*=Ajustes do teste neste computador')).toBeDisplayed();
    await expect($('[role="menuitem"]*=Recriar instância de teste')).toBeDisplayed();
    await screenshot('41-menu-testar');
    await browser.keys('Escape');
    await $('[role="menuitem"]*=Ver último teste').waitForDisplayed({ reverse: true });
  });

  it('CA-T13-08: Testar abre o jogo; o botão leva de volta à tela do teste a partir de Mods', async () => {
    await testButton().click();
    await waitHeading('Jogo aberto');
    const console = $('section[aria-label="Console do jogo"]');
    await browser.waitUntil(async () => (await console.getText()).includes('pai vivo'), {
      timeout: 30_000,
      timeoutMsg: 'o console não mostrou o jogo simulado',
    });
    await expect(console).toHaveText(expect.stringContaining('Abrindo o jogo: 1.20.1'));
    await expect(testButton()).toHaveText(expect.stringContaining('Jogo aberto: ver teste'));
    await screenshot('42-jogo-aberto-console');

    // A instância recebeu o pack.
    const game = instanceGameDir();
    expect(readFileSync(join(game, 'config', 'vale.toml'), 'utf8')).toBe('dificuldade = 2\n');

    await $('nav[aria-label="Seções do pack"]').$('a*=Mods').click();
    await waitHeading('Mods');
    await screenshot('43-botao-jogo-aberto');
    await testButton().click();
    await waitHeading('Jogo aberto');
  });

  it('CA-T13-02: Parar jogo encerra o jogo em até 5 s, com "Encerrado por você"', async () => {
    await $('main').$('button=Parar jogo').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await screenshot('44-parar-jogo');
    const asked = Date.now();
    await dialog.$('button=Parar jogo').click();
    await waitHeading('Encerrado por você', 15_000);
    expect(Date.now() - asked).toBeLessThan(5_000);
    await expect(testButton()).toHaveText('Testar');
    await screenshot('45-resultado-encerrado');
  });

  it('CA-T13-06: um travamento termina em "O jogo travou" com "Por que travou"', async () => {
    setFakeGame('crash');
    await $('main').$('button=Testar de novo').click();
    await waitHeading('O jogo travou');
    const why = $('section[aria-labelledby="why-crashed-title"]');
    await expect(why).toHaveText(expect.stringContaining('Por que travou'));
    await expect(why).toHaveText(expect.stringContaining('O jogo fechou com o código'));
    await expect(why).toHaveText(expect.stringContaining('crash report'));
    await screenshot('46-resultado-travou');
  });

  it('CA-T13-03 e CA-T13-04: acentos certos no console; o mod tirado sai da instância e o mundo fica', async () => {
    const game = instanceGameDir();
    mkdirSync(join(game, 'saves', 'Mundo de teste'), { recursive: true });
    writeFileSync(join(game, 'saves', 'Mundo de teste', 'level.dat'), 'mundo');
    writePack({ 'config/vale.toml': 'dificuldade = 2\n' }, 'acentos');

    // O crash report do teste anterior conta como novo por 2 s (folga do relógio de arquivos
    // da `warden-launcher`): espera passar antes de testar de novo.
    await browser.pause(3000);
    await $('main').$('button=Testar de novo').click();
    await waitHeading('O jogo fechou normalmente');
    await $('main').$('button=Ver console').click();
    const console = $('section[aria-label="Console do jogo"]');
    await expect(console).toHaveText(expect.stringContaining('Ação, coração, pé, avô, vovó, pão'));
    await expect(console).toHaveText(expect.stringContaining('Configuração inválida: ç ã õ é'));
    expect(await console.getText()).not.toContain('Ã');
    await screenshot('47-console-acentos');

    expect(existsSync(join(game, 'mods', 'biblioteca.jar'))).toBe(false);
    expect(readFileSync(join(game, 'saves', 'Mundo de teste', 'level.dat'), 'utf8')).toBe('mundo');
  });

  it('Salvar em arquivo… grava o log da sessão', async () => {
    const out = join(DATA_ROOT, 'console-salvo.log');
    writeFileSync(SAVE_FILE, out);
    await $('section[aria-label="Console do jogo"]').$('button=Salvar em arquivo…').click();
    await browser.waitUntil(() => existsSync(out), { timeout: 15_000 });
    expect(readFileSync(out, 'utf8')).toContain('coração');
  });

  it('CA-T13-08: "Ver último teste" e os testes anteriores', async () => {
    await $('nav[aria-label="Seções do pack"]').$('a*=Mods').click();
    await waitHeading('Mods');
    await $('header.packhead .testbtn__more').click();
    await $('[role="menuitem"]*=Ver último teste').click();
    await waitHeading('O jogo fechou normalmente');
    const table = $('main table');
    await table.waitForDisplayed();
    await expect(table.$$('tbody tr')).toBeElementsArrayOfSize(3);
    await expect(table).toHaveText(expect.stringContaining('travou'));
    await expect(table).toHaveText(expect.stringContaining('encerrado por você'));
    await screenshot('48-testes-anteriores');
    await table.$('a*=hoje').click();
  });
});
