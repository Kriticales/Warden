/**
 * Teclado e foco nas telas P0 do app real (A-01; SPEC §7.6; HANDOFF §7), com o pack que o
 * packwiz real gerou para o Fabric 1.21.1 (o mesmo do `editor.e2e.ts`) e as respostas gravadas
 * do Modrinth (nada vai à internet).
 *
 * - Foco visível: em cada tela, o Tab passa por todos os controles e cada um que recebe o foco
 *   mostra o contorno de foco (`outline` não recortado por `clip-path`), no WebView de verdade:
 *   o jsdom dos testes de componente não calcula estilos.
 * - Ordem do Tab: a ordem segue a do documento (nada de `tabIndex` positivo), começa pelo
 *   cabeçalho e não prende o foco em lugar nenhum fora de diálogos.
 * - Diálogos devolvem o foco: a confirmação aberta pelo menu ⋯ de Meus packs volta ao ⋯, e os
 *   detalhes do mod abertos pela linha voltam à linha (os dois abrem por estado, sem `Trigger`).
 * - axe sem violação séria em cada tela visitada.
 */
import { cpSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { focusTrace, startFocusTrace } from './focus-trace';
import { openOrImport } from './open-or-import';
import { dismissToasts } from './toasts';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const FIXTURE = join(
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
const PACK = join(DATA_ROOT, 'fora', 'pack-foco');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

/** Quantos Tabs, no máximo, por tela (as listas longas têm um botão por linha). */
const MAX_STOPS = 60;

/** Um ponto de parada do Tab. */
interface Stop {
  /** Descrição curta para a mensagem de erro: tag, nome e classe. */
  label: string;
  /** O contorno de foco aparece (não está desligado nem recortado). */
  visible: boolean;
  /** O foco caiu no `<body>` (saiu da página). */
  body: boolean;
}

/** Aperta Tab e descreve onde o foco caiu. */
async function tab(): Promise<Stop> {
  await browser.keys('Tab');
  return browser.execute(() => {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || element === document.body) {
      return { label: 'body', visible: false, body: true };
    }
    // O contorno vem do `:focus-visible` (design system) no próprio elemento ou, nas caixas e
    // opções de rádio desenhadas, no rótulo em volta (`.choice:has(input:focus-visible)`).
    const shows = (node: Element | null): boolean => {
      if (!node) return false;
      const style = getComputedStyle(node);
      const outlined = style.outlineStyle !== 'none' && Number.parseFloat(style.outlineWidth) > 0;
      return outlined && style.clipPath === 'none';
    };
    const name =
      element.getAttribute('aria-label') ??
      element.textContent.trim().replace(/\s+/g, ' ').slice(0, 40);
    return {
      label: `${element.tagName.toLowerCase()}.${element.className} "${name}"`,
      visible: shows(element) || shows(element.parentElement),
      body: false,
    };
  });
}

/**
 * Passa o Tab pela tela inteira (até voltar ao primeiro controle ou `MAX_STOPS`) e devolve os
 * controles sem contorno de foco visível. O foco nunca cai no `<body>` no meio do caminho.
 */
async function tabThroughPage(): Promise<string[]> {
  await dismissToasts();
  // Começa do topo: o "Pular para o conteúdo" é o primeiro controle de toda tela.
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur();
  });
  const missing: string[] = [];
  let visited = 0;
  let first: string | null = null;
  for (let count = 0; count < MAX_STOPS; count += 1) {
    const stop = await tab();
    if (stop.body) {
      // O Tab saiu da página (fim do documento): o próximo volta ao começo.
      if (first !== null) break;
      continue;
    }
    if (stop.label === first) break;
    first ??= stop.label;
    visited += 1;
    if (!stop.visible) missing.push(stop.label);
  }
  // O ciclo termina de volta no "Pular para o conteúdo", que aparece por cima do cabeçalho
  // enquanto tem o foco: tira o foco para o próximo clique do teste não cair nele.
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur();
  });
  // A tela tem mais do que a moldura (pular, cabeçalho e rodapé): o Tab passou pelo conteúdo.
  expect(visited).toBeGreaterThan(5);
  return missing;
}

/** A linha da lista de Mods de um item, pelo nome. */
function modRow(name: string) {
  return $(`tr.modrow*=${name}`);
}

/** Vai para Meus packs, de onde estiver (o voltar do pack e o das Configurações levam a `/packs`). */
async function goToPacks(): Promise<void> {
  await dismissToasts();
  if (!(await $('h1=Meus packs').isExisting())) {
    let clicked = false;
    for (const link of await $$('a[href="/packs"]').getElements()) {
      if (await link.isDisplayed()) {
        await link.click();
        clicked = true;
        break;
      }
    }
    if (!clicked) throw new Error('nenhum link visível para Meus packs');
  }
  await $('h1=Meus packs').waitForDisplayed({ timeout: 30_000 });
}

/**
 * Garante o pack do teste na lista de Meus packs, abrindo-o por "Abrir ou importar…" se ainda
 * não estiver lá: cada caso parte dele sem depender de o anterior ter chegado ao fim.
 */
async function ensurePackListed(): Promise<void> {
  await goToPacks();
  const open = $('button[aria-label="Abrir Fixture Fabric"]');
  if (await open.isExisting()) return;
  writeFileSync(PICK_FILE, PACK);
  await openOrImport();
  await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
  await $('button=Abrir pack').click();
  await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
  await modRow('Sodium').waitForDisplayed();
  await goToPacks();
  await open.waitForDisplayed();
}

/** Garante o pack aberto (em qualquer seção). */
async function ensureInPack(): Promise<void> {
  if (await $('nav[aria-label="Seções do pack"]').isExisting()) return;
  await ensurePackListed();
  await $('button[aria-label="Abrir Fixture Fabric"]').click();
  await $('h1=Mods').waitForDisplayed({ timeout: 30_000 });
}

/**
 * Fecha o que um caso que falhou deixou aberto (menu, diálogo, painel), para não derrubar os
 * seguintes: Esc algumas vezes e, se nem assim, recarrega a página (o registro dos packs fica).
 */
async function closeStrayLayers(): Promise<void> {
  const open = () => $('[role="menu"], [role="dialog"], [role="alertdialog"]').isExisting();
  for (let attempt = 0; attempt < 3 && (await open()); attempt += 1) {
    await browser.keys('Escape');
    await browser.pause(300);
  }
  if (await open()) {
    await browser.refresh();
    await $('main h1').waitForDisplayed({ timeout: 30_000 });
  }
}

/** Espera `condition`; se não vier, falha com `message` e o registro de foco e camadas. */
async function expectWithTrace(condition: () => Promise<boolean>, message: string): Promise<void> {
  try {
    await browser.waitUntil(condition, { timeout: 5_000 });
  } catch {
    throw new Error(`${message}\n${await focusTrace()}`);
  }
}

/** Vai para uma seção do pack pelo menu lateral e espera o título. */
async function openSection(name: string): Promise<void> {
  await dismissToasts();
  await ensureInPack();
  await $('nav[aria-label="Seções do pack"]').$(`a*=${name}`).click();
  await $(`h1=${name}`).waitForDisplayed({ timeout: 30_000 });
}

/** Põe o foco no botão com este texto (o nome do ⋯ está num texto só para leitor de tela). */
async function focusButton(text: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((wanted: string) => {
        const button = Array.from(document.querySelectorAll<HTMLElement>('button')).find(
          (candidate) => candidate.textContent.trim() === wanted,
        );
        button?.focus();
        return button !== undefined && document.activeElement === button;
      }, text),
    { timeout: 15_000, timeoutMsg: `o botão "${text}" não apareceu` },
  );
}

/** O elemento focado agora, descrito como no `tab()`. */
async function focusedLabel(): Promise<string> {
  return browser.execute(() => {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || element === document.body) return 'body';
    return element.getAttribute('aria-label') ?? element.textContent.trim();
  });
}

describe('Teclado e foco nas telas P0 (A-01)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    mkdirSync(join(DATA_ROOT, 'fora'), { recursive: true });
    cpSync(FIXTURE, PACK, { recursive: true });
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
  });

  afterEach(async () => {
    await closeStrayLayers();
  });

  after(() => {
    // Devolve a pasta de dados vazia para os specs seguintes.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
  });

  it('Meus packs vazio: foco visível em todos os controles', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    expect(await tabThroughPage()).toEqual([]);
  });

  it('abre o pack e, em Meus packs, a confirmação aberta pelo ⋯ devolve o foco ao ⋯', async () => {
    await ensurePackListed();
    expect(await tabThroughPage()).toEqual([]);
    expect(await seriousViolationSummaries()).toEqual([]);

    // Pelo teclado, como quem não usa o mouse: foco no ⋯, Enter abre, ↓ escolhe, Enter confirma.
    await focusButton('Mais ações para Fixture Fabric');
    await browser.keys('Enter');
    await $('[role="menu"]').waitForDisplayed();
    await browser.waitUntil(
      async () => {
        const label = await focusedLabel();
        if (label.includes('Remover da lista')) return true;
        await browser.keys('ArrowDown');
        return false;
      },
      { timeout: 10_000, timeoutMsg: '"Remover da lista" não recebeu o foco no menu' },
    );
    await startFocusTrace();
    await browser.keys('Enter');
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    expect(await seriousViolationSummaries()).toEqual([]);
    await browser.keys('Escape');
    await expectWithTrace(
      async () => !(await dialog.isExisting()),
      'a confirmação não fechou com Esc',
    );
    await expectWithTrace(
      async () => (await focusedLabel()) === 'Mais ações para Fixture Fabric',
      'o foco não voltou ao ⋯ depois de fechar a confirmação',
    );
  });

  it('Mods: foco visível, e os detalhes abertos pela linha devolvem o foco à linha', async () => {
    await ensurePackListed();
    await $('button[aria-label="Abrir Fixture Fabric"]').click();
    await $('h1=Mods').waitForDisplayed({ timeout: 30_000 });
    await modRow('Sodium').waitForDisplayed();
    expect(await tabThroughPage()).toEqual([]);

    await dismissToasts();
    await focusButton('Sodium');
    await startFocusTrace();
    await browser.keys('Enter');
    const drawer = $('[role="dialog"]');
    await drawer.waitForDisplayed();
    expect(await seriousViolationSummaries()).toEqual([]);
    await browser.keys('Escape');
    await expectWithTrace(
      async () => !(await drawer.isExisting()),
      'os detalhes não fecharam com Esc',
    );
    await expectWithTrace(
      async () => (await focusedLabel()) === 'Sodium',
      'o foco não voltou à linha do Sodium depois de fechar os detalhes',
    );
  });

  for (const section of ['Configs', 'Problemas', 'Histórico', 'Exportar']) {
    it(`${section}: foco visível em todos os controles; axe`, async () => {
      await openSection(section);
      // Espera a seção terminar de ler (nada carregando).
      await browser.waitUntil(
        async () => !(await $('main [aria-busy="true"], main .loader').isExisting()),
        { timeout: 60_000, timeoutMsg: `${section} não terminou de carregar` },
      );
      expect(await tabThroughPage()).toEqual([]);
      expect(await seriousViolationSummaries()).toEqual([]);
    });
  }

  it('Configurações: foco visível em todos os controles; axe', async () => {
    await goToPacks();
    await $('header').$('a=Configurações').click();
    await $('h1=Configurações').waitForDisplayed();
    await $('h2=Geral').waitForDisplayed();
    expect(await tabThroughPage()).toEqual([]);
    expect(await seriousViolationSummaries()).toEqual([]);
  });
});
