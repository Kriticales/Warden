/**
 * Buscar em todas as configs no app real (C-05; SPEC T12), com o índice de verdade. O pack é o
 * que o packwiz real gerou para o Fabric 1.21.1 (o mesmo do `configs.e2e.ts`), mais configs
 * reais do corpus da `warden-configs` (TOML do Forge, `.cfg` do Forge antigo, JSON5) e uma com
 * a chave `spawnRate`.
 *
 * - CA-T12-06: "spawn" acha "Spawn" e `spawnRate` sem diferenciar maiúsculas, com os resultados
 *   agrupados por arquivo; clicar abre o arquivo no editor na linha da chave.
 * - CA-T12-07: editar um arquivo por fora atualiza os resultados sem reiniciar o app (o tempo
 *   medido sai no log do teste; o limite de 2 s é conferido no teste de domínio).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { dismissToasts } from './toasts';

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
const CRATES = join(import.meta.dirname, '..', '..', '..', 'crates');
const FIXTURE = join(
  CRATES,
  'warden-packwiz',
  'tests',
  'fixtures',
  'packwiz-output',
  'fabric-1.21.1',
);
const CORPUS = join(CRATES, 'warden-configs', 'tests', 'corpus');
const PACK = join(DATA_ROOT, 'fora', 'pack-busca');
const MOBS = join(PACK, 'config', 'mobs-common.toml');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

/** Configs reais do corpus que entram no pack (com "Spawn" em chaves e comentários). */
const REAL = [
  ['cfg', '01-Universal-Tweaks---Bugfixes.cfg', 'Universal-Tweaks---Bugfixes.cfg'],
  ['cfg', '04-UniversalTweaks.cfg', 'UniversalTweaks.cfg'],
  ['toml', '02-forge-client.toml', 'forge-client.toml'],
  ['json5', '01-appleskin.json5', 'appleskin.json5'],
] as const;

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
}

async function seriousViolations(): Promise<string[]> {
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
  return violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id}: ${v.help}`);
}

/** O título do painel de resultados quando ele diz `text`. */
async function waitTitle(text: string, timeout = 30_000): Promise<void> {
  let seen = '';
  await browser.waitUntil(
    async () => {
      const title = $('#cfg-search-title');
      seen = (await title.isExisting()) ? await title.getText() : '';
      return seen.includes(text);
    },
    { timeout, interval: 50, timeoutMsg: `o título não disse "${text}" (disse: "${seen}")` },
  );
}

/** Troca o texto do campo de busca. `setValue` digita tecla por tecla, como a pessoa (o
 * `browser.keys` do WebKitWebDriver perde letras repetidas, como o "ee" de "creeperzinho"). */
async function search(text: string): Promise<void> {
  const box = $('input[aria-label="Buscar em todas as configs"]');
  await box.click();
  await box.setValue(text);
  await expect(box).toHaveValue(text);
}

/** Os arquivos com resultado, na ordem da tela. */
async function resultFiles(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('.cfg-search .hits__group')).map(
      (group) => group.getAttribute('aria-label') ?? '',
    ),
  );
}

describe('Buscar em todas as configs (C-05; T12)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    for (const [folder, file, name] of REAL) {
      cpSync(join(CORPUS, folder, file), join(PACK, 'config', name));
    }
    writeFileSync(MOBS, '[mobs]\n\t#Chance de um mob aparecer\n\tspawnRate = 5\n');
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

  it('CA-T12-06: acha "Spawn" e spawnRate agrupados por arquivo e abre na linha; axe', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed();
    writeFileSync(PICK_FILE, PACK);
    await $('button=Abrir ou importar…').click();
    await $('h1=Abrir pack').waitForDisplayed();
    await $('button=Abrir pack').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
    await dismissToasts();
    await $('nav[aria-label="Seções do pack"]').$('a*=Configs').click();
    await $('h1=Configs').waitForDisplayed();
    await $('[role="tree"]').waitForDisplayed();

    await search('spawn');
    await waitTitle('para “spawn”');
    const files = await resultFiles();
    expect(files).toContain('config/mobs-common.toml');
    expect(files).toContain('config/UniversalTweaks.cfg');
    // Agrupado: cada arquivo aparece uma vez.
    expect(new Set(files).size).toBe(files.length);
    const panel = $('.cfg-search');
    const text = await panel.getText();
    expect(text).toContain('spawnRate');
    expect(text).toContain('Spawn');
    expect(await panel.$$('mark').length).toBeGreaterThan(files.length);
    await screenshot('44-configs-busca');
    expect(await seriousViolations()).toEqual([]);

    // Clicar abre o arquivo no editor, na linha da chave.
    await panel.$('section[aria-label="config/mobs-common.toml"]').$('button*=spawnRate').click();
    await expect($('.editor__file')).toHaveText('config/mobs-common.toml');
    await expect($('.editor__foot')).toHaveText(expect.stringContaining('linha 3, coluna 1'));
    await expect($('.cfg-search')).not.toBeExisting();
  });

  it('CA-T12-07: editar um arquivo por fora atualiza os resultados sem reiniciar', async () => {
    await search('creeperzinho');
    await waitTitle('Nenhuma config com “creeperzinho”.');

    const started = Date.now();
    writeFileSync(MOBS, '[mobs]\n\t#Chance de um mob aparecer\n\tcreeperzinhoRate = 5\n');
    await waitTitle('1 resultado em 1 arquivo para “creeperzinho”', 10_000);
    // O tempo medido vai para o relatório da tarefa.
    process.stdout.write(
      `C-05: edição por fora apareceu na busca em ${String(Date.now() - started)} ms\n`,
    );
    expect(await resultFiles()).toEqual(['config/mobs-common.toml']);

    // E a chave antiga sumiu.
    await search('spawnRate');
    await browser.waitUntil(
      async () => !(await resultFiles()).includes('config/mobs-common.toml'),
      {
        timeout: 10_000,
        timeoutMsg: 'a chave antiga continuou nos resultados',
      },
    );
    await screenshot('45-configs-busca-atualizada');
  });
});
