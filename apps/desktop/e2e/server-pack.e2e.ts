/**
 * Pacote para servidor no app real (E-03; SPEC T19, CA-T19-07 e CA-T19-08), com o jogo e o
 * servidor simulados (o mesmo `WardenFakeServer` do "Testar como servidor"): o app não baixa
 * Java nem servidor; a geração, o zip, a validação no servidor local e a tela são os de
 * verdade. O conteúdo das duas variantes com mods de referência, o bootstrap real e os scripts
 * rodando até "Done" são provados no Rust (`crates/warden-export/tests/server_pack`).
 *
 * - Sem publicação: "Baixar os mods pelo link do pack" fica desabilitado com o motivo, e a
 *   variante com mods dentro pede a confirmação de licença do jar local.
 * - A validação começa desmarcada (EULA não aceita); marcar pergunta a EULA e o aceite marca.
 * - CA-T19-08: o pacote correto termina com "Pacote para servidor testado" e o tempo até "Done";
 *   o zip leva os scripts (start.bat com CRLF, start.sh com LF), as configs e o jar local, e não
 *   leva o "Só cliente", `options.txt`, `kubejs/client_scripts/` nem `eula.txt`.
 * - CA-T19-08: com o servidor derrubado por um mod de cliente, a validação mostra "O servidor
 *   travou com este pacote", o mod apontado e o trecho do log.
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from './axe';
import { dismissToasts } from './toasts';
import { zipEntries, zipEntry } from './zip';

const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const BINARIES = join(import.meta.dirname, '..', 'src-tauri', 'binaries');
const PACK = join(DATA_ROOT, 'fora', 'pack-pacote-servidor');
const OUT = join(DATA_ROOT, 'pacote-servidor');
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

function packwizBinary(): string {
  const suffix = process.platform === 'win32' ? 'windows-msvc.exe' : 'linux-gnu';
  const name = readdirSync(BINARIES).find(
    (entry) =>
      entry.startsWith('packwiz-') &&
      entry.endsWith(suffix) &&
      !entry.includes('.build') &&
      !entry.endsWith('.commit'),
  );
  if (!name) throw new Error('sidecar do packwiz ausente: rode cargo xtask build-packwiz');
  return join(BINARIES, name);
}

function write(relative: string, text: string): void {
  const file = join(PACK, relative);
  mkdirSync(join(file, '..'), { recursive: true });
  writeFileSync(file, text);
}

/** Um pack vanilla 1.20.1 com configs, scripts, um jar local e um mod "Só cliente". */
function writePack(): void {
  rmSync(PACK, { recursive: true, force: true });
  mkdirSync(PACK, { recursive: true });
  write(
    'pack.toml',
    [
      'name = "Vale do Pacote"',
      'version = "1.0.0"',
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      'hash = ""',
      '',
      '[versions]',
      'minecraft = "1.20.1"',
      '',
    ].join('\n'),
  );
  write('index.toml', 'hash-format = "sha256"\n');
  write('config/vale.toml', 'dificuldade = 2\n');
  write('kubejs/server_scripts/servidor.js', '// servidor\n');
  write('kubejs/client_scripts/cliente.js', '// cliente\n');
  write('options.txt', 'fov:90\n');
  write('mods/zoomcliente-1.0.jar', 'jar de cliente marcado como dos dois lados');
  write('mods/so-cliente.pw.toml', CLIENT_ONLY);
  execFileSync(packwizBinary(), ['refresh'], { cwd: PACK, stdio: 'pipe' });
  // Os modos do jogo e do servidor simulados ficam fora do índice.
  write('.warden-fake-game', 'normal');
  write('.warden-fake-server', 'normal');
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

async function openServerPackFormat(): Promise<void> {
  await $('nav[aria-label="Seções do pack"]').$('a*=Exportar').click();
  await $('h1=Exportar').waitForDisplayed();
  await $('h2=2. Formato').waitForDisplayed({ timeout: 60_000 });
  await $('label*=Pacote para servidor (.zip)').click();
  await $('h2=Como os mods chegam ao servidor').waitForDisplayed({ timeout: 60_000 });
}

/** Clica em Gerar e espera o resultado (a validação abre o servidor simulado). */
async function generate(destination: string) {
  writeFileSync(PICK_FILE, destination);
  await dismissToasts();
  const button = $('button=Gerar pacote para servidor…');
  await button.waitForClickable({ timeout: 30_000 });
  await button.click();
  const dialog = $('[role="dialog"]*=Pacote para servidor gerado');
  await dialog.waitForDisplayed({ timeout: 180_000 });
  return dialog;
}

describe('Pacote para servidor (E-03; T19)', function () {
  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): E2E do pacote para servidor pulado');
      this.skip();
    }
    rmSync(OUT, { recursive: true, force: true });
    mkdirSync(OUT, { recursive: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    writePack();
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('sem publicação: o link fica desabilitado e os mods dentro pedem a licença; a validação pede a EULA', async () => {
    await step('abrir o pack', async () => {
      await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 20_000 });
      writeFileSync(PICK_FILE, PACK);
      await $('button=Abrir ou importar…').click();
      await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed({ timeout: 20_000 });
      // A leitura do pack termina com "Abrir pack" ou, antes, com uma pergunta ("Agora não").
      const open = $('button=Abrir pack');
      const notNow = $('button=Agora não');
      await browser.waitUntil(
        async () => (await open.isExisting()) || (await notNow.isExisting()),
        { timeout: 60_000, timeoutMsg: 'Abrir pack não ficou pronto' },
      );
      if (await notNow.isExisting()) await notNow.click();
      else await open.click();
      await $('h1=Mods').waitForDisplayed({ timeout: 30_000 });
    });
    await step('formato', openServerPackFormat);
    const link = $('label*=Baixar os mods pelo link do pack');
    await expect(link).toHaveText(expect.stringContaining('ainda não foi publicado'));
    await expect(link.$('input')).toBeDisabled();
    await expect(link.$('input')).not.toBeSelected();
    await expect($('label*=Mods dentro do zip').$('input')).toBeSelected();
    const contents = $('section[aria-labelledby="sp-conteudo"]');
    await expect(contents).toHaveText(
      expect.stringContaining('1 mod só de cliente fica de fora: Zoom de Cliente.'),
    );
    await expect(contents).toHaveText(
      expect.stringContaining('kubejs/: 1 arquivo (sem client_scripts/)'),
    );
    await expect($('button=Gerar pacote para servidor…')).toBeDisabled();
    expect(await seriousViolations()).toEqual([]);

    await step('licença', async () => {
      await $('label*=Tenho o direito de distribuir estes arquivos').click();
      await expect($('button=Gerar pacote para servidor…')).toBeEnabled();
    });
    await step('EULA', async () => {
      const box = $('label*=Testar no servidor deste computador depois de gerar').$('input');
      await expect(box).not.toBeSelected();
      await $('label*=Testar no servidor deste computador depois de gerar').click();
      const eula = $('[role="dialog"]*=Aceitar a EULA do Minecraft?');
      await eula.waitForDisplayed({ timeout: 15_000 });
      await eula.$('button=Aceito a EULA').click();
      await eula.waitForDisplayed({ reverse: true, timeout: 15_000 });
      await expect(box).toBeSelected();
    });
  });

  it('CA-T19-08: o pacote correto termina testado, com o tempo até "Done", e o zip leva só o que o servidor usa', async () => {
    const out = join(OUT, 'vale-do-pacote-servidor.zip');
    const dialog = await generate(out);
    await expect(dialog).toHaveText(
      expect.stringContaining('Pacote para servidor testado: o servidor abriu em'),
    );
    await expect(dialog).toHaveText(expect.stringContaining('Com 1 mod dentro do zip.'));
    expect(await seriousViolations()).toEqual([]);

    const entries = zipEntries(out);
    expect(entries).toEqual(
      expect.arrayContaining([
        'start.bat',
        'start.sh',
        'user_jvm_args.txt',
        'config/vale.toml',
        'kubejs/server_scripts/servidor.js',
        'mods/zoomcliente-1.0.jar',
      ]),
    );
    for (const absent of [
      'eula.txt',
      'options.txt',
      'kubejs/client_scripts/cliente.js',
      'mods/so-cliente.jar',
      'mods/so-cliente.pw.toml',
      '.warden-fake-server',
    ]) {
      expect(entries).not.toContain(absent);
    }
    const bat = zipEntry(out, 'start.bat');
    const sh = zipEntry(out, 'start.sh');
    if (!bat || !sh) throw new Error('scripts ausentes');
    expect(
      bat
        .toString('utf8')
        .split('\n')
        .slice(0, -1)
        .every((line) => line.endsWith('\r')),
    ).toBe(true);
    expect(sh.includes('\r')).toBe(false);
    await dialog.$('button=Fechar').click();
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });

  it('CA-T19-08: um mod de cliente marcado como dos dois lados reprova a validação com o mod e o log', async () => {
    write('.warden-fake-server', 'travar-cliente');
    const dialog = await generate(join(OUT, 'vale-do-pacote-travou.zip'));
    await expect(dialog).toHaveText(expect.stringContaining('O servidor travou com este pacote.'), {
      wait: 15_000,
    });
    await expect(dialog).toHaveText(expect.stringContaining('zoomcliente'));
    await expect(dialog).toHaveText(expect.stringContaining('Trecho do log do servidor'));
    await expect(dialog).not.toHaveText(expect.stringContaining('Pacote para servidor testado'));
    await dialog.$('button=Fechar').click();
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });
});
