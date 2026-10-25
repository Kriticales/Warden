/**
 * Os 5 fluxos avançados do protótipo (D4), de ponta a ponta no app real (A-07; ROADMAP A-07 e
 * o "Mapa e fluxos" de `design/prototipo-final/app.js`, fluxos 6 a 10). Cada fluxo junta, na
 * ordem em que a pessoa faz, telas que os E2E de cada tarefa provam separadas; aqui o que vale
 * é a passagem de uma tela para a outra, os dados atualizados depois de cada mudança, o teclado
 * e o `axe` em cada tela.
 *
 * 1. Descobrir e adicionar vários mods: a página de descoberta só pelo teclado (busca, marcar
 *    dois resultados das duas fontes, abrir a pré-visualização), um diálogo de dependências
 *    para os dois, a gravação numa vez só e a lista de Mods com os três itens.
 * 2. Trazer mods de um modpack: Tipo = Modpacks, o "Pacote Leve", dois mods marcados, um
 *    diálogo só, "Já no pack" no modpack e a lista de Mods com os dois.
 * 3. Travou sem causa: o menu ▾ do Testar abre a busca do culpado, que acha o `charlie.jar`;
 *    "Ver o que o charlie.jar altera no jogo" abre o raio-x nos detalhes; a conversa com a IA
 *    (Gemini simulado) leva o que a pessoa contou e a resposta aparece com o "Enviado à IA".
 * 4. Testar como servidor e gerar o pacote para servidor: o menu ▾ só pelo teclado (o foco
 *    volta ao ▾ quando o diálogo da EULA fecha), o servidor local com o jogo dentro, os
 *    formatos de Exportar (Prism e .mrpack), o pacote para servidor testado no servidor local e,
 *    com o pack parado, o aviso de mudança feita fora do Warden.
 * 5. Importar um modpack: o `.mrpack` por "Abrir ou importar…", a revisão, o pack novo aberto em
 *    Mods, o aviso em Problemas e a linha em Meus packs; Configurações → Armazenamento no fim.
 *
 * Servidores simulados (nada vai à internet): Modrinth, CurseForge (com a chave de teste),
 * Gemini, o jogo e o servidor simulados da `warden-launcher` e da `warden-server`. Os fluxos 3
 * e 4 precisam de Java (`WARDEN_TEST_JAVA` ou `JAVA_HOME`); sem ele, avisam e pulam.
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava as capturas das telas.
 */
import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { CHAVE_BOA } from './mock-server/curseforge';
import { GEMINI_E2E_KEY } from './mock-server/gemini';
import { storedZip } from './mock-server/jars';
import { openOrImport } from './open-or-import';
import { dismissToasts } from './toasts';
import { zipEntries } from './zip';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
const HAS_JAVA = process.env.WARDEN_E2E_HAS_JAVA === '1';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FOLDER = env('WARDEN_E2E_PICK_FOLDER');
const MOCK_URL = env('WARDEN_E2E_MOCK_URL');
const OUTSIDE = join(DATA_ROOT, 'fora', 'fluxos-avancados');
const PACKS = join(DATA_ROOT, 'packs');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');
const VAULT = join(DATA_ROOT, 'cofre-de-teste');
const CURSEFORGE_KEY_FILE = join(VAULT, 'curseforge-api-key');
const GEMINI_KEY_FILE = join(VAULT, 'gemini-api-key');
/** O pack que o packwiz real gerou para o Fabric 1.21.1 (Lithium, Sodium, Fabric API…). */
const FABRIC_FIXTURE = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  'crates',
  'warden-packwiz',
  'tests',
  'fixtures',
  'packwiz-output',
  'fabric-1.21.1',
);

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

function write(path: string, content: string | Buffer): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content);
}

function read(pack: string, path: string): string {
  return readFileSync(join(pack, path), 'utf8').replace(/\r\n/g, '\n');
}

/** Os packs registrados em Meus packs. */
function registeredPacks(): { id: string; path: string }[] {
  if (!existsSync(REGISTRY_FILE)) return [];
  const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
    packs?: { id: string; path: string }[];
  };
  return registry.packs ?? [];
}

/** A pasta de dados como os outros E2E esperam: sem packs, chaves, instâncias nem cache. */
function cleanDataRoot(): void {
  for (const pack of registeredPacks()) {
    rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
  }
  rmSync(SETTINGS_FILE, { force: true });
  rmSync(REGISTRY_FILE, { force: true });
  rmSync(CURSEFORGE_KEY_FILE, { force: true });
  rmSync(GEMINI_KEY_FILE, { force: true });
  rmSync(PACKS, { recursive: true, force: true });
  rmSync(OUTSIDE, { recursive: true, force: true });
  rmSync(join(DATA_ROOT, 'cache'), { recursive: true, force: true });
  writeFileSync(PICK_FOLDER, '');
}

/** Começa um fluxo com o app recém-aberto, Meus packs vazio e as Configurações de fábrica. */
async function freshApp(): Promise<void> {
  cleanDataRoot();
  mkdirSync(OUTSIDE, { recursive: true });
  write(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
  await browser.reloadSession();
  await browser.setWindowSize(1366, 860);
}

/** Copia o pack Fabric 1.21.1 sem o Sodium (para ele aparecer como algo a adicionar). */
function copyFabricPack(pack: string, withoutSodium: boolean): void {
  rmSync(pack, { recursive: true, force: true });
  cpSync(FABRIC_FIXTURE, pack, { recursive: true });
  if (!withoutSodium) return;
  rmSync(join(pack, 'mods', 'sodium.pw.toml'));
  const index = read(pack, 'index.toml').replace(
    /\[\[files\]\]\nfile = "mods\/sodium\.pw\.toml"\nhash = "[0-9a-f]+"\nmetafile = true\n\n/,
    '',
  );
  writeFileSync(join(pack, 'index.toml'), index);
  writeFileSync(
    join(pack, 'pack.toml'),
    read(pack, 'pack.toml').replace(/^hash = "[0-9a-f]+"$/m, `hash = "${sha256(index)}"`),
  );
}

/** Um pack com os arquivos dados no índice (sha256) e o resto do `pack.toml`. */
function writeIndexedPack(
  pack: string,
  name: string,
  versions: string[],
  files: Record<string, string | Buffer>,
): void {
  rmSync(pack, { recursive: true, force: true });
  mkdirSync(pack, { recursive: true });
  const entries = Object.entries(files)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, content]) => {
      write(join(pack, path), content);
      const metafile = path.endsWith('.pw.toml') ? 'metafile = true\n' : '';
      return `[[files]]\nfile = "${path}"\nhash = "${sha256(content)}"\n${metafile}`;
    });
  const index = `hash-format = "sha256"\n\n${entries.join('\n')}`;
  writeFileSync(join(pack, 'index.toml'), index);
  writeFileSync(
    join(pack, 'pack.toml'),
    [
      `name = "${name}"`,
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
      ...versions,
      '',
    ].join('\n'),
  );
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await dismissToasts();
  await browser.saveScreenshot(join(SCREENSHOTS, `a07-${name}.png`));
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

/** Nenhuma violação séria ou crítica do `axe` na tela de agora. */
async function expectAccessible(): Promise<void> {
  expect(await seriousViolationSummaries()).toEqual([]);
}

async function waitHeading(text: string, timeout = 60_000): Promise<void> {
  await $(`h1*=${text}`).waitForDisplayed({ timeout });
}

/** Texto completo de um seletor, inclusive o que está fora da tela ou recolhido. */
async function fullText(selector: string): Promise<string> {
  return browser.execute((sel) => document.querySelector(sel)?.textContent ?? '', selector);
}

/** Clica depois de fechar os avisos e de o alvo parar de mexer. */
async function clickStable(element: ReturnType<typeof $>): Promise<void> {
  await dismissToasts();
  await element.waitForClickable({ timeout: 30_000 });
  await element.waitForStable({ timeout: 10_000 });
  await element.click();
}

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

/** Abre uma pasta de pack por "Abrir ou importar…" (Meus packs vazio) e espera Mods. */
async function openPackFolder(pack: string): Promise<void> {
  await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
  writeFileSync(PICK_FOLDER, pack);
  await openOrImport();
  await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed({ timeout: 20_000 });
  const open = $('button=Abrir pack');
  const notNow = $('button=Agora não');
  await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
    timeout: 60_000,
    timeoutMsg: 'Abrir pack não ficou pronto',
  });
  if (await notNow.isExisting()) await notNow.click();
  else await open.click();
  await waitHeading('Mods');
}

/** A descrição curta do elemento com o foco: rótulo acessível ou texto. */
async function focusedName(): Promise<string> {
  return browser.execute(() => {
    const element = document.activeElement;
    if (!element || element === document.body) return '';
    return (element.getAttribute('aria-label') ?? element.textContent).trim();
  });
}

/**
 * Aperta a tecla até o foco chegar ao elemento cujo nome passa em `wanted` (no máximo
 * `limit` vezes). Falha dizendo por onde o foco passou.
 */
async function pressUntilFocused(
  key: string,
  wanted: (name: string) => boolean,
  limit = 60,
): Promise<void> {
  const visited: string[] = [];
  for (let i = 0; i < limit; i++) {
    await browser.keys(key);
    const name = await focusedName();
    visited.push(name);
    if (wanted(name)) return;
  }
  throw new Error(`o foco não chegou ao alvo com ${key}; passou por: ${visited.join(' | ')}`);
}

/** O elemento com o foco está com o foco visível (`:focus-visible` e um contorno desenhado). */
async function expectFocusVisible(): Promise<void> {
  const visible = await browser.execute(() => {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || !element.matches(':focus-visible')) return false;
    const style = getComputedStyle(element);
    const outline = style.outlineStyle !== 'none' && Number.parseFloat(style.outlineWidth) > 0;
    return outline || style.boxShadow !== 'none';
  });
  expect(visible).toBe(true);
}

async function mockRequests(): Promise<string[]> {
  const response = await fetch(`${MOCK_URL}/__requests`);
  return (await response.json()) as string[];
}

describe('Fluxo 6: descobrir e adicionar vários mods (A-07)', () => {
  const pack = join(OUTSIDE, 'pack-descobrir');

  before(async () => {
    await freshApp();
    copyFabricPack(pack, true);
    write(CURSEFORGE_KEY_FILE, CHAVE_BOA);
    await fetch(`${MOCK_URL}/__reset`, { method: 'POST' });
  });

  after(cleanDataRoot);

  it('a página de descoberta abre com populares e passa no axe', async () => {
    await step('abrir o pack', async () => {
      await openPackFolder(pack);
      await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
    });
    await step('Adicionar', async () => {
      await clickStable($('a=Adicionar'));
      await $('h1=Adicionar ao pack').waitForDisplayed();
      await $('h2=Populares para Minecraft 1.21.1 com Fabric').waitForDisplayed({
        timeout: 30_000,
      });
      await $('h2=Atualizados recentemente').waitForDisplayed();
    });
    await expectAccessible();
    await screenshot('06-1-descoberta');
  });

  it('só pelo teclado: busca, marca dois resultados e abre a pré-visualização', async () => {
    await step('buscar pelo teclado', async () => {
      const field = $('input[type="search"]');
      await field.click();
      // Digitado no campo (`addValue`): o `browser.keys` aperta todas as teclas de uma vez (todas
      // para baixo, depois todas para cima), e o texto chegava sem o segundo "m" ("sodium od").
      await field.addValue('sodium mod');
      // O início também lista o Sodium e o Mod Só da CurseForge, e a lista é redesenhada do zero
      // a cada busca nova: só vale a lista da busca com o texto inteiro, dita pelo estado do
      // campo ("N resultados para “sodium mod”"). Antes disso, o Tab cairia numa linha que some.
      await $('.omnibox__status*=resultados para “sodium mod”').waitForDisplayed({
        timeout: 30_000,
      });
      const results = $('ul[aria-label="Resultados"]');
      await results.$('button.drow__name=Mod Só da CurseForge').waitForDisplayed();
      await results.$('button.drow__name=Sodium').waitForDisplayed();
      await expect($('h2=Populares para Minecraft 1.21.1 com Fabric')).not.toBeExisting();
      await dismissToasts();
      // O foco volta ao campo, como se a pessoa tivesse acabado de digitar.
      await browser.execute(() => {
        document.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
      });
    });
    await step('marcar o Sodium com Espaço', async () => {
      await pressUntilFocused('Tab', (name) => name === 'Selecionar Sodium');
      await expectFocusVisible();
      await browser.keys(' ');
      await $('.selbar*=1 selecionado').waitForDisplayed();
    });
    await step('marcar o Mod Só da CurseForge com Espaço', async () => {
      await pressUntilFocused('Tab', (name) => name === 'Selecionar Mod Só da CurseForge');
      await browser.keys(' ');
      await $('.selbar*=2 selecionados').waitForDisplayed();
    });
    await step('abrir a pré-visualização do Sodium com Enter', async () => {
      await browser.execute(() => {
        document.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
      });
      await pressUntilFocused('Tab', (name) => name === 'Sodium');
      await expectFocusVisible();
      await browser.keys('Enter');
      await $('aside.disc__preview').$('h2=Sodium').waitForDisplayed({ timeout: 30_000 });
      await browser.waitUntil(
        async () => (await fullText('aside.disc__preview')).includes('(mais nova compatível)'),
        { timeout: 30_000, timeoutMsg: 'a pré-visualização não mostrou a versão' },
      );
    });
    await expectAccessible();
    await screenshot('06-2-selecao-previa');
  });

  it('um diálogo só para os dois, com a dependência; grava tudo e a lista de Mods mostra os três', async () => {
    await step('Adicionar 2 ao pack', async () => {
      await clickStable($('button=Adicionar 2 ao pack'));
      await $('[role="dialog"]').waitForDisplayed({ timeout: 30_000 });
      await browser.waitUntil(
        async () => (await fullText('[role="dialog"]')).includes('Biblioteca da CurseForge'),
        { timeout: 30_000, timeoutMsg: 'o plano com a dependência não chegou' },
      );
      expect(await $$('[role="dialog"]').length).toBe(1);
      const plan = await fullText('[role="dialog"]');
      expect(plan).toContain('Sodium');
      expect(plan).toContain('Mod Só da CurseForge');
      expect(plan).toContain('Obrigatórias');
    });
    await expectAccessible();
    await screenshot('06-3-dependencias');
    await step('confirmar', async () => {
      await clickStable($('[role="dialog"]').$('button=Adicionar 3 itens'));
      await $('.toast*=3 itens adicionados ao pack').waitForDisplayed({ timeout: 60_000 });
      await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 30_000 });
    });
    expect(read(pack, 'mods/sodium.pw.toml')).toContain('[update.modrinth]');
    expect(read(pack, 'mods/mod-so-da-curseforge.pw.toml')).toContain('metadata:curseforge');
    expect(read(pack, 'mods/biblioteca-curseforge.pw.toml')).toContain('metadata:curseforge');
    await step('a descoberta marca "Já no pack" e a volta para Mods mostra os três', async () => {
      await $(
        '//li[contains(@class, "drow")][.//button[normalize-space(.)="Sodium"]]//*[contains(., "Já no pack")]',
      ).waitForDisplayed({ timeout: 30_000 });
      await clickStable($('a*=Voltar para Mods · 3 adicionados'));
      await waitHeading('Mods');
      for (const name of ['Sodium', 'Mod Só da CurseForge', 'Biblioteca da CurseForge']) {
        await $(`tr.modrow*=${name}`).waitForDisplayed({ timeout: 30_000 });
      }
    });
    // As duas fontes foram consultadas (a CurseForge com a chave do cofre de teste).
    const lines = await mockRequests();
    expect(lines.some((line) => line.includes('/curseforge/'))).toBe(true);
    expect(lines.some((line) => line.includes('/modrinth/v2/search'))).toBe(true);
    await expectAccessible();
  });
});

describe('Fluxo 7: trazer mods de outro modpack (A-07)', () => {
  const pack = join(OUTSIDE, 'pack-modpack');
  const panel = 'aside[aria-label="Mods do modpack Pacote Leve"]';

  /** O texto da linha de um mod no painel do modpack. */
  async function modRow(name: string): Promise<string> {
    return browser.execute(
      (sel, wanted) =>
        Array.from(document.querySelectorAll(`${sel} li.drow`)).find(
          (row) => row.querySelector('.modpack__name')?.textContent === wanted,
        )?.textContent ?? '',
      panel,
      name,
    );
  }

  before(async () => {
    await freshApp();
    copyFabricPack(pack, false);
  });

  after(cleanDataRoot);

  it('Tipo = Modpacks abre o Pacote Leve com o que já está no pack; passa no axe', async () => {
    await step('abrir o pack e Adicionar', async () => {
      await openPackFolder(pack);
      await $('tr.modrow*=Lithium').waitForDisplayed({ timeout: 30_000 });
      await clickStable($('a=Adicionar'));
      await $('h1=Adicionar ao pack').waitForDisplayed();
    });
    await step('Tipo = Modpacks', async () => {
      await $('select[aria-label="Tipo"]').selectByAttribute('value', 'modpack');
      const open = $('button[aria-label="Ver os mods de Pacote Leve"]');
      await open.waitForDisplayed({ timeout: 30_000 });
      await clickStable(open);
      await $(panel).$('h2=Mods deste modpack (6)').waitForDisplayed({ timeout: 30_000 });
    });
    expect(await modRow('Sodium')).toContain('Já no pack');
    expect(await modRow('spark')).not.toContain('Já no pack');
    expect(await modRow('ajustes-do-autor-1.0')).toContain('Arquivo fora das lojas');
    await expectAccessible();
    await screenshot('07-1-modpack');
  });

  it('dois mods marcados entram num diálogo só e aparecem como "Já no pack" e em Mods', async () => {
    await step('marcar e adicionar', async () => {
      await clickStable($(`${panel} input[aria-label="Selecionar spark"]`));
      await clickStable($(`${panel} input[aria-label="Selecionar Crash Assistant"]`));
      await clickStable(
        $(
          '//aside[@aria-label="Mods do modpack Pacote Leve"]//button[normalize-space(.)="Adicionar 2 selecionados"]',
        ),
      );
      await $('[role="dialog"]').waitForDisplayed({ timeout: 30_000 });
      await browser.waitUntil(
        async () => (await fullText('[role="dialog"]')).includes('O que você escolheu'),
        { timeout: 30_000, timeoutMsg: 'o plano não chegou' },
      );
      expect(await $$('[role="dialog"]').length).toBe(1);
    });
    await expectAccessible();
    await screenshot('07-2-dependencias');
    await step('confirmar', async () => {
      await clickStable(
        $('//div[@role="dialog"]//button[starts-with(normalize-space(.), "Adicionar ")]'),
      );
      await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 60_000 });
    });
    await step('o modpack relido mostra os dois no pack', async () => {
      await browser.waitUntil(
        async () =>
          (await modRow('spark')).includes('Já no pack') &&
          (await modRow('Crash Assistant')).includes('Já no pack'),
        { timeout: 30_000, timeoutMsg: 'o modpack não mostrou os itens adicionados' },
      );
    });
    // Gravado com a versão que o modpack usa (spark `cALUj9l1`).
    expect(read(pack, 'mods/spark.pw.toml')).toContain('cALUj9l1');
    await step('a lista de Mods mostra os dois', async () => {
      await clickStable($('a*=Voltar para Mods'));
      await waitHeading('Mods');
      await $('tr.modrow*=spark').waitForDisplayed({ timeout: 30_000 });
      await $('tr.modrow*=Crash Assistant').waitForDisplayed({ timeout: 30_000 });
    });
  });
});

describe('Fluxo 8: travou sem causa, o culpado e a conversa com a IA (A-07)', function () {
  const pack = join(OUTSIDE, 'pack-culpado');
  const mods = ['alfa', 'bravo', 'charlie', 'delta', 'echo'];
  const note = 'Trava sempre que entro no mundo; a busca apontou o charlie.jar.';

  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): fluxo 8 do A-07 pulado');
      this.skip();
    }
    await freshApp();
    const files: Record<string, Buffer> = {};
    for (const id of mods) {
      files[`mods/${id}.jar`] = storedZip({
        'fabric.mod.json': `{"schemaVersion":1,"id":"${id}","version":"1.0","depends":{}}`,
      });
    }
    writeIndexedPack(pack, 'Vale Sem Causa', ['minecraft = "1.20.1"', 'fabric = "0.16.10"'], files);
    // O jogo simulado entra no mundo e falha ao colocar o jogador quando o `charlie` está ligado.
    write(join(pack, '.warden-fake-game'), 'culpado 600 charlie');
    write(GEMINI_KEY_FILE, GEMINI_E2E_KEY);
    await fetch(`${MOCK_URL}/__gemini/reset`, { method: 'POST' });
  });

  after(cleanDataRoot);

  it('o menu ▾ do Testar abre a busca do culpado, que acha o charlie.jar', async () => {
    await step('abrir o pack', async () => {
      await openPackFolder(pack);
    });
    await step('Encontrar o mod culpado…', async () => {
      await clickStable($('header.packhead .testbtn__more'));
      await clickStable($('[role="menuitem"]*=Encontrar o mod culpado…'));
      const dialog = $('[role="dialog"]');
      await dialog.$('h2=Encontrar o mod culpado').waitForDisplayed({ timeout: 30_000 });
      await dialog.$('span*=5 mods entram na busca.').waitForDisplayed({ timeout: 30_000 });
      await clickStable(dialog.$('button=Aceito a EULA'));
      await dialog.$('span*=Antes da primeira rodada, o Warden cria').waitForDisplayed();
      await expectAccessible();
      await screenshot('08-1-dialogo');
      await clickStable(dialog.$('label*=Trava ao entrar no mundo'));
      await clickStable(dialog.$('button=Começar a busca'));
      await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
    });
    await step('a busca roda e acha o culpado', async () => {
      await waitHeading('Buscando o mod culpado');
      await waitHeading('Culpado encontrado: charlie.jar', 180_000);
      await expect($('main')).toHaveText(
        expect.stringContaining('Alta: confirmado em 2 testes de controle'),
      );
      await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    });
    await expectAccessible();
    await screenshot('08-2-resultado');
  });

  it('"Ver o que o charlie.jar altera no jogo" abre o raio-x nos detalhes do mod', async () => {
    await step('abrir os detalhes', async () => {
      await clickStable($('button=Ver o que o charlie.jar altera no jogo'));
      await waitHeading('Mods');
      const details = $('[role="dialog"]*=charlie.jar');
      await details.waitForDisplayed({ timeout: 30_000 });
      await details.$('h3*=O que este mod altera no jogo').waitForExist({ timeout: 30_000 });
      await details
        .$('p*=Este mod não altera o código do jogo por mixins.')
        .waitForExist({ timeout: 30_000 });
    });
    await expectAccessible();
    await screenshot('08-3-raio-x');
    await browser.keys('Escape');
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 10_000 });
  });

  it('a conversa com a IA leva o que a pessoa contou e mostra a resposta com o que foi enviado', async () => {
    await step('Diagnóstico com IA', async () => {
      await clickStable(sectionLink('Diagnóstico com IA'));
      await waitHeading('Diagnóstico com IA');
      await $('h2=Nova conversa').waitForDisplayed({ timeout: 30_000 });
      await clickStable($('label*=Trava ao entrar no mundo'));
      const source = $('select.select');
      await source.waitForEnabled({ timeout: 30_000 });
      await source.selectByAttribute('value', 'pack');
      const field = $('textarea');
      await field.click();
      await field.addValue(note);
    });
    await step('consentimento', async () => {
      await clickStable($('main').$('button=Começar conversa'));
      const dialog = $('[role="dialog"]');
      await dialog.$('h2=Começar uma conversa com a IA?').waitForDisplayed({ timeout: 30_000 });
      const text = await fullText(
        '[role="dialog"] pre[aria-label="Texto inicial que será enviado"]',
      );
      expect(text).toContain(note);
      await expectAccessible();
      await screenshot('08-4-consentimento');
      await clickStable(dialog.$('button=Começar conversa'));
      await dialog.waitForDisplayed({ reverse: true, timeout: 30_000 });
    });
    await step('a resposta', async () => {
      await $('div*=O que a IA afirma').waitForDisplayed({ timeout: 60_000 });
      await expect($$('details.toolcall')).toBeElementsArrayOfSize({ gte: 3 });
    });
    const state = (await (await fetch(`${MOCK_URL}/__gemini`)).json()) as {
      generate: string[];
      refusals: string[];
    };
    expect(state.refusals).toEqual([]);
    expect(state.generate.length).toBe(2);
    expect(state.generate[0]).toContain(note);
    await expectAccessible();
    await screenshot('08-5-conversa');
  });
});

describe('Fluxo 9: testar como servidor e gerar o pacote para servidor (A-07)', function () {
  const pack = join(OUTSIDE, 'pack-servidor');
  const out = join(OUTSIDE, 'pacote');

  before(async function () {
    if (!HAS_JAVA) {
      // eslint-disable-next-line no-console -- aviso de teste pulado
      console.warn('sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): fluxo 9 do A-07 pulado');
      this.skip();
    }
    await freshApp();
    writeIndexedPack(pack, 'Vale do Servidor Local', ['minecraft = "1.20.1"'], {
      'config/vale.toml': 'dificuldade = 2\n',
    });
    write(join(pack, '.warden-fake-game'), 'conectar');
    write(join(pack, '.warden-fake-server'), 'normal');
    mkdirSync(out, { recursive: true });
  });

  after(cleanDataRoot);

  it('o menu ▾ só pelo teclado; o foco volta ao ▾ quando o diálogo fecha', async () => {
    await step('abrir o pack', async () => {
      await openPackFolder(pack);
      await dismissToasts();
    });
    const more = $('header.packhead .testbtn__more');
    const openServerItem = async () => {
      // Como a pessoa: o foco no botão Testar e Tab até o ▾ (o foco vindo do teclado é o
      // que desenha o contorno).
      await browser.execute(() => {
        document.querySelector<HTMLElement>('header.packhead .testbtn__main')?.focus();
      });
      await browser.keys('Tab');
      await browser.waitUntil(
        async () =>
          browser.execute((element) => document.activeElement === element, await more.getElement()),
        { timeout: 5_000, timeoutMsg: 'o Tab não levou o foco ao ▾ do Testar' },
      );
      await expectFocusVisible();
      await browser.keys('Enter');
      await $('[role="menu"]').waitForDisplayed({ timeout: 10_000 });
      await pressUntilFocused('ArrowDown', (name) => name.startsWith('Testar como servidor'), 20);
      await expectAccessible();
      await browser.keys('Enter');
    };
    await step('Testar como servidor… pelo teclado e Esc', async () => {
      await openServerItem();
      const eula = $('[role="dialog"]');
      await eula.$('h2=Aceitar a EULA do Minecraft?').waitForDisplayed({ timeout: 15_000 });
      await browser.keys('Escape');
      await eula.waitForExist({ reverse: true, timeout: 10_000 });
      await browser.waitUntil(
        async () =>
          browser.execute(() => document.activeElement?.classList.contains('testbtn__more')),
        { timeout: 5_000, timeoutMsg: 'o foco não voltou ao ▾ do Testar' },
      );
    });
    await step('aceitar a EULA e abrir o servidor', async () => {
      await openServerItem();
      const eula = $('[role="dialog"]');
      await eula.$('h2=Aceitar a EULA do Minecraft?').waitForDisplayed({ timeout: 15_000 });
      await clickStable(eula.$('button=Aceito a EULA'));
      const options = $('[role="dialog"]');
      await options.$('h2=Testar como servidor').waitForDisplayed({ timeout: 15_000 });
      await expectAccessible();
      await screenshot('09-1-opcoes');
      const start = options.$('button=Abrir servidor e testar');
      await start.waitForEnabled({ timeout: 15_000 });
      await clickStable(start);
    });
  });

  it('o servidor local abre, o jogo entra e "Parar jogo e servidor" fecha os dois', async () => {
    await step('servidor e jogo', async () => {
      await waitHeading('Testando como servidor');
      const console = $('section[aria-label="Console do servidor"]');
      await browser.waitUntil(async () => (await console.getText()).includes('joined the game'), {
        timeout: 60_000,
        timeoutMsg: 'o jogo não entrou no servidor',
      });
    });
    await expectAccessible();
    await screenshot('09-2-servidor');
    await step('parar', async () => {
      await clickStable($('main').$('button=Parar jogo e servidor'));
      const confirm = $('[role="alertdialog"]');
      await confirm.waitForDisplayed();
      await clickStable(confirm.$('button=Parar jogo e servidor'));
      await waitHeading('Encerrado por você', 90_000);
      await expect($('section[aria-labelledby="server-result-title"]')).toHaveText(
        expect.stringContaining('Entrou no servidor.'),
      );
      await expect($('header.packhead .testbtn__main')).toHaveText('Testar');
    });
  });

  it('Exportar: Prism e .mrpack passam no axe; o pacote para servidor sai testado', async () => {
    await step('Exportar e os formatos', async () => {
      await clickStable(sectionLink('Exportar'));
      await $('h1=Exportar').waitForDisplayed();
      await $('h2=2. Formato').waitForDisplayed({ timeout: 60_000 });
      await clickStable($('label*=Instância pronta para o Prism'));
      await $('h2=O que vai no arquivo').waitForDisplayed({ timeout: 60_000 });
      await expectAccessible();
      await clickStable($('label*=.mrpack'));
      await $('h2=O que este formato perde').waitForDisplayed({ timeout: 60_000 });
      await expectAccessible();
      await clickStable($('label*=Pacote para servidor (.zip)'));
      await $('h2=Como os mods chegam ao servidor').waitForDisplayed({ timeout: 60_000 });
    });
    await step('validação já marcada (EULA aceita)', async () => {
      const box = $('label*=Testar no servidor deste computador depois de gerar').$('input');
      await expect(box).toBeSelected();
    });
    await expectAccessible();
    await screenshot('09-3-pacote');
    const zip = join(out, 'vale-do-servidor-local-servidor.zip');
    await step('gerar', async () => {
      writeFileSync(PICK_FOLDER, zip);
      await clickStable($('button=Gerar pacote para servidor…'));
      const dialog = $('[role="dialog"]*=Pacote para servidor gerado');
      await dialog.waitForDisplayed({ timeout: 180_000 });
      await expect(dialog).toHaveText(
        expect.stringContaining('Pacote para servidor testado: o servidor abriu em'),
      );
      await expectAccessible();
      await screenshot('09-4-gerado');
      await clickStable(dialog.$('button=Fechar'));
      await $('[role="dialog"]').waitForExist({ reverse: true });
    });
    const entries = zipEntries(zip);
    expect(entries).toEqual(expect.arrayContaining(['start.bat', 'start.sh', 'config/vale.toml']));
    expect(entries).not.toContain('eula.txt');
    expect(entries).not.toContain('.warden-fake-server');
  });

  it('uma mudança feita fora do Warden aparece no aviso e no cabeçalho do pack', async () => {
    await step('renomear o pack por fora', async () => {
      writeFileSync(
        join(pack, 'pack.toml'),
        read(pack, 'pack.toml').replace(/^name = ".*"$/m, 'name = "Vale Renomeado por Fora"'),
      );
      await $('.toast*=O pack foi alterado fora do Warden').waitForDisplayed({ timeout: 20_000 });
      await expectAccessible();
      await dismissToasts();
      await $('h1=Vale Renomeado por Fora').waitForDisplayed({ timeout: 20_000 });
    });
  });
});

describe('Fluxo 10: importar um modpack de outro app (A-07)', () => {
  /** O Sodium gravado nas fixtures do Modrinth (versão `SMxNOGZ6`). */
  const sodium = {
    path: 'mods/sodium-fabric-0.8.13+mc1.21.1.jar',
    url: 'https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar',
    sha1: '003c114c85ca88ef3362e018deb6aca0c682d6a1',
    sha512:
      '8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e54e8be06d5ea5a9c268beb0e02073209d1f831a02dede851588b972367680853',
  };

  before(freshApp);

  after(cleanDataRoot);

  it('o .mrpack vira um pack novo, aberto em Mods, com o aviso em Problemas e a linha em Meus packs', async () => {
    const origin = join(OUTSIDE, 'Vale Importado 3.1.0.mrpack');
    write(
      origin,
      storedZip({
        'modrinth.index.json': JSON.stringify({
          formatVersion: 1,
          game: 'minecraft',
          versionId: '3.1.0',
          name: 'Vale Importado',
          dependencies: { minecraft: '1.21.1', 'fabric-loader': '0.16.5' },
          files: [
            {
              path: sodium.path,
              hashes: { sha1: sodium.sha1, sha512: sodium.sha512 },
              env: { client: 'required', server: 'unsupported' },
              downloads: [sodium.url],
              fileSize: 1574609,
            },
          ],
        }),
        'overrides/config/sodium-options.json': '{ "quality": {} }\n',
        'overrides/mods/caseiro.jar': 'jar feito em casa',
      }),
    );
    await step('Abrir ou importar… → Escolher arquivo…', async () => {
      await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
      writeFileSync(PICK_FOLDER, origin);
      await openOrImport('arquivo');
      await waitHeading('Importar modpack', 20_000);
    });
    await step('revisão', async () => {
      const button = $('button=Importar como pack novo');
      await button.waitForDisplayed({ timeout: 60_000 });
      await browser.waitUntil(async () => button.isEnabled(), {
        timeout: 20_000,
        timeoutMsg: 'Importar como pack novo não ficou disponível',
      });
      await expect($('main')).toHaveText(
        expect.stringContaining('Vale Importado 3.1.0.mrpack · Minecraft 1.21.1 · Fabric 0.16.5'),
      );
      await expect($('main')).toHaveText(expect.stringContaining('1 jar fica como arquivo local'));
    });
    await expectAccessible();
    await screenshot('10-1-revisao');
    await step('importar', async () => {
      await clickStable($('button=Importar como pack novo'));
      await waitHeading('Mods', 90_000);
      await $('tr.modrow*=Sodium').waitForDisplayed({ timeout: 30_000 });
    });
    await step('Problemas mostra o item para revisar', async () => {
      await clickStable(sectionLink('Problemas'));
      await waitHeading('Problemas', 20_000);
      const notice = $('.alert*=para revisar');
      await notice.waitForDisplayed({ timeout: 20_000 });
      await expect(notice).toHaveText(expect.stringContaining('caseiro.jar'));
    });
    await expectAccessible();
    await screenshot('10-2-problemas');
    await step('Meus packs mostra o pack novo', async () => {
      await dismissToasts();
      await clickStable($('header.packhead').$('a*=Meus packs'));
      await $('h1=Meus packs').waitForDisplayed({ timeout: 20_000 });
      await $('button[aria-label="Abrir Vale Importado"]').waitForDisplayed({ timeout: 20_000 });
    });
    expect(readdirSync(PACKS)).toContain('vale-importado');
    expect(existsSync(join(PACKS, 'vale-importado', 'mods', 'sodium.pw.toml'))).toBe(true);
  });

  it('Configurações → Armazenamento mostra o espaço usado e passa no axe', async () => {
    await clickStable($('header.topbar').$('a*=Configurações'));
    await $('h1*=Geral').waitForDisplayed({ timeout: 30_000 });
    await clickStable($('nav.sidenav').$('a*=Armazenamento'));
    await $('h1*=Armazenamento').waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(async () => !(await fullText('main')).includes('Medindo…'), {
      timeout: 30_000,
      timeoutMsg: 'o espaço usado não terminou de ser medido',
    });
    await expect($('main')).toHaveText(expect.stringContaining('Instâncias de teste'));
    await expectAccessible();
    await screenshot('10-3-armazenamento');
  });
});
