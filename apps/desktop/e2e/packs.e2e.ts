/**
 * Meus packs, Criar pack e Abrir pack no app real (P1-07; SPEC T02, T03 e T04), com o packwiz
 * de verdade, o git de verdade e o catálogo pelas respostas gravadas (servidor de fixtures).
 *
 * - Criar pack Forge 1.20.1 pelo assistente: a pasta tem `pack.toml` com as versões escolhidas
 *   e o repositório git, e o pack abre.
 * - Abrir um pack packwiz com três arquivos que não deveriam ir para os jogadores: "Limpar"
 *   apaga os três (CA-T04-01) e registra o pack com `.warden/project.toml`.
 * - Pasta renomeada por fora vira "Pasta não encontrada"; "Localizar…" com a pasta nova
 *   restaura a linha (CA-T02-02).
 * - "Remover da lista" não muda nenhum arquivo (hash da pasta antes e depois; CA-T02-03).
 *
 * O diálogo nativo de pasta, que o WebDriver não alcança, é trocado pelo arquivo de
 * `WARDEN_E2E_PICK_FOLDER` (`wdio.conf.ts`). Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as
 * capturas das telas.
 */
import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { join, relative } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');
const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const OLD_PACK = join(DATA_ROOT, 'fora', 'pack-antigo');
const MOVED_PACK = join(DATA_ROOT, 'fora', 'pack-antigo-movido');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/** Um ULID válido e único por número (identificador de pack). */
function ulid(n: number): string {
  let tail = '';
  let rest = n;
  for (let index = 0; index < 8; index += 1) {
    tail = (CROCKFORD[rest % 32] ?? '0') + tail;
    rest = Math.floor(rest / 32);
  }
  return `01JB${'0'.repeat(14)}${tail}`;
}

/** O que o "diálogo" de pasta vai devolver na próxima escolha. */
function pickNext(folder: string): void {
  writeFileSync(PICK_FILE, folder);
}

/** Um pack packwiz NeoForge 1.20.1 com três arquivos que não deveriam ir para os jogadores. */
function writeOldPack(dir: string): void {
  mkdirSync(join(dir, 'config'), { recursive: true });
  writeFileSync(join(dir, 'index.toml'), 'hash-format = "sha256"\n');
  writeFileSync(
    join(dir, 'pack.toml'),
    [
      'name = "Pack Antigo"',
      'author = "Teste"',
      'version = "1.2.0"',
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      '',
      '[versions]',
      'minecraft = "1.20.1"',
      'neoforge = "47.1.106"',
      '',
    ].join('\n'),
  );
  writeFileSync(join(dir, 'config', 'x.toml.bak'), 'antigo = true\n');
  writeFileSync(join(dir, 'packwiz-installer-bootstrap.jar'), 'não é um jar de verdade');
  writeFileSync(join(dir, '.packwiz.toml'), '# arquivo de outro programa\n');
}

/** Hash de todos os arquivos de uma pasta (caminho + conteúdo), para "nada mudou". */
function folderHash(dir: string): string {
  const hash = createHash('sha256');
  const walk = (current: string) => {
    for (const entry of readdirSync(current).sort()) {
      const path = join(current, entry);
      if (statSync(path).isDirectory()) {
        walk(path);
      } else {
        hash.update(relative(dir, path));
        hash.update(readFileSync(path));
      }
    }
  };
  walk(dir);
  return hash.digest('hex');
}

interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
}

async function seriousViolations(): Promise<AxeViolation[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async () => {
    const { axe } = window as unknown as {
      axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
    };
    try {
      return (await axe.run({ resultTypes: ['violations'] })).violations;
    } catch {
      const failed: AxeViolation = {
        id: 'axe-falhou',
        impact: 'critical',
        help: 'o axe não rodou',
      };
      return [failed];
    }
  });
  return violations.filter((v) => v.impact === 'serious' || v.impact === 'critical');
}

async function backToList(): Promise<void> {
  // "← Meus packs" fica na barra do app ou, dentro de um pack (P1-08), no cabeçalho do pack.
  await $('header.topbar, header.packhead').$('a*=Meus packs').click();
  await $('h1=Meus packs').waitForDisplayed();
}

/** Um item do menu aberto, pelo texto. */
function menuItem(text: string) {
  return $('[role="menu"]').$(`div*=${text}`);
}

/** Clica no botão primário do assistente ("Próximo" ou "Criar pack"). */
async function wizardNext(label = 'Próximo'): Promise<void> {
  const button = $('.wizard__foot').$(`button*=${label}`);
  await button.waitForEnabled();
  await button.click();
}

describe('Meus packs, Criar pack e Abrir pack', () => {
  before(async () => {
    writeOldPack(OLD_PACK);
    pickNext('');
    // Sem settings.json o app abre na primeira execução (P1-13, coberta em settings.e2e.ts):
    // grava as configurações padrão e reabre o app (sessão nova = processo novo).
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  after(() => {
    // Devolve a pasta de dados sem configurações e sem packs registrados: o settings.e2e.ts,
    // que roda depois (ordem alfabética), começa pela primeira execução.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(join(DATA_ROOT, 'config', 'packs.json'), { force: true });
  });

  it('abre em Meus packs vazio, sem violação séria do axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    await screenshot('01-meus-packs-vazio');
    const violations = await seriousViolations();
    expect(violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });

  it('cria um pack Forge 1.20.1 pelo assistente, com o catálogo de versões', async () => {
    await $('button=Criar pack').click();
    await $('h1=Criar pack').waitForDisplayed();
    const nameId = await $('label=Nome do pack').getAttribute('for');
    await $(`[id="${nameId ?? ''}"]`).setValue('Vale Sereno');
    // A pasta acompanha o nome, dentro da pasta dos packs.
    const folder = $('input.input--mono');
    await browser.waitUntil(async () => (await folder.getValue()).endsWith('vale-sereno'));
    await screenshot('02-criar-nome-e-pasta');
    await wizardNext();

    const search = $('input[type="search"]');
    await search.waitForDisplayed();
    await screenshot('03-criar-versao-do-minecraft');
    await search.setValue('1.20.1');
    await $('label.check*=1.20.1').click();
    await wizardNext();

    const forge = $('.choice*=Forge');
    await forge.waitForDisplayed();
    await browser.waitUntil(() => $('.choice input:checked').isExisting());
    await expect($('.choice*=Quilt')).toExist();
    await screenshot('04-criar-loader');
    await wizardNext();

    await $('.packcard__meta*=Minecraft 1.20.1 · Forge').waitForDisplayed();
    await screenshot('05-criar-resumo');
    await wizardNext('Criar pack');

    await $('h1=Vale Sereno').waitForDisplayed({ timeout: 60_000 });
    const created = join(DATA_ROOT, 'packs', 'vale-sereno');
    const manifest = readFileSync(join(created, 'pack.toml'), 'utf8');
    expect(manifest).toContain('minecraft = "1.20.1"');
    expect(manifest).toMatch(/forge = "47\.[\d.]+"/);
    expect(existsSync(join(created, '.git'))).toBe(true);
    expect(existsSync(join(created, '.warden', 'project.toml'))).toBe(true);
    await screenshot('06-pack-criado');
  });

  it('abre um pack packwiz existente e limpa os três arquivos (CA-T04-01)', async () => {
    await backToList();
    pickNext(OLD_PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('h2*=3 arquivos não deveriam ir para quem joga o pack').waitForDisplayed();
    await expect($('td.path*=config/x.toml.bak')).toExist();
    await expect($('td.path*=packwiz-installer-bootstrap.jar')).toExist();
    await expect($('td.path*=.packwiz.toml')).toExist();
    // Verificar não escreve nada.
    expect(existsSync(join(OLD_PACK, '.warden'))).toBe(false);
    await screenshot('07-abrir-pack-higiene');
    const violations = await seriousViolations();
    expect(violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);

    await $('button=Limpar 3 arquivos e abrir').click();
    await $('h1=Pack Antigo').waitForDisplayed({ timeout: 60_000 });
    expect(existsSync(join(OLD_PACK, 'config', 'x.toml.bak'))).toBe(false);
    expect(existsSync(join(OLD_PACK, 'packwiz-installer-bootstrap.jar'))).toBe(false);
    expect(existsSync(join(OLD_PACK, '.packwiz.toml'))).toBe(false);
    expect(existsSync(join(OLD_PACK, '.warden', 'project.toml'))).toBe(true);
    expect(readFileSync(join(OLD_PACK, '.packwizignore'), 'utf8')).toContain('/.warden/');
  });

  it('lista os dois packs com versões e o menu ⋯', async () => {
    await backToList();
    await expect($$('tr.packrow')).toBeElementsArrayOfSize(2);
    await expect($('tr.packrow*=Vale Sereno')).toHaveText(
      expect.stringContaining('Minecraft 1.20.1 · Forge'),
    );
    await expect($('tr.packrow*=Pack Antigo')).toHaveText(
      expect.stringContaining('Minecraft 1.20.1 · NeoForge 47.1.106'),
    );
    await screenshot('08-meus-packs');
    await $('button*=Mais ações para Vale Sereno').click();
    await $('[role="menu"]').waitForDisplayed();
    await screenshot('09-meus-packs-menu');
    await browser.keys('Escape');
    await $('[role="menu"]').waitForDisplayed({ reverse: true });
  });

  it('pasta renomeada por fora vira "Pasta não encontrada"; Localizar… restaura (CA-T02-02)', async () => {
    renameSync(OLD_PACK, MOVED_PACK);
    await browser.refresh();
    await $('h1=Meus packs').waitForDisplayed();
    const missing = $('tr.packrow--missing*=Pasta não encontrada');
    await missing.waitForDisplayed();
    await screenshot('10-pasta-nao-encontrada');
    pickNext(MOVED_PACK);
    await missing.$('button=Localizar…').click();
    await $('button[aria-label="Abrir Pack Antigo"]').waitForDisplayed();
    await expect($('tr.packrow--missing')).not.toExist();
  });

  it('"Remover da lista" não muda nenhum arquivo (CA-T02-03)', async () => {
    const before = folderHash(MOVED_PACK);
    await $('button*=Mais ações para Pack Antigo').click();
    await menuItem('Remover da lista').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await screenshot('11-remover-da-lista');
    await dialog.$('button=Remover da lista').click();
    await $('button[aria-label="Abrir Pack Antigo"]').waitForDisplayed({ reverse: true });
    expect(folderHash(MOVED_PACK)).toBe(before);
    await expect($$('tr.packrow')).toBeElementsArrayOfSize(1);
  });

  it('em 1024 px (largura mínima) a lista não estoura na horizontal', async () => {
    await browser.setWindowSize(1024, 700);
    await browser.pause(300);
    const overflow = await browser.execute(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBe(0);
    await screenshot('12-meus-packs-1024');
  });

  it('CA-T02-01: com 50 packs registrados, a lista aparece em menos de 1 s', async () => {
    // Registra 50 packs no packs.json e reabre o app (nova sessão = novo processo).
    const registryFile = join(DATA_ROOT, 'config', 'packs.json');
    const registry = JSON.parse(readFileSync(registryFile, 'utf8')) as {
      packs: { id: string; name: string; path: string }[];
    };
    for (let index = 1; index <= 50; index += 1) {
      const dir = join(DATA_ROOT, 'cinquenta', `pack-${String(index)}`);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, 'index.toml'), 'hash-format = "sha256"\n');
      writeFileSync(
        join(dir, 'pack.toml'),
        [
          `name = "Pack ${String(index)}"`,
          'pack-format = "packwiz:1.1.0"',
          '',
          '[index]',
          'file = "index.toml"',
          'hash-format = "sha256"',
          '',
          '[versions]',
          'minecraft = "1.20.1"',
          '',
        ].join('\n'),
      );
      registry.packs.push({ id: ulid(index), name: `Pack ${String(index)}`, path: dir });
    }
    writeFileSync(registryFile, JSON.stringify(registry, null, 2));
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
    // Do início da página até as 51 linhas na tela, medido dentro do WebView.
    const shownAtMs = await browser.execute(
      () =>
        new Promise<number>((resolve) => {
          const check = () => {
            if (document.querySelectorAll('tr.packrow').length >= 51) resolve(performance.now());
            else requestAnimationFrame(check);
          };
          check();
        }),
    );
    process.stdout.write(`CA-T02-01: lista com 51 packs em ${shownAtMs.toFixed(0)} ms\n`);
    expect(shownAtMs).toBeLessThan(1000);
    await screenshot('13-meus-packs-51');
  });
});
