/**
 * "Relatar problema" no app real (UI-02, redesenhado na UI-03):
 *
 * - CA-UI02-01 e CA-UI03-08: o botão (no Linux, no rodapé) abre o diálogo; sem título, "Abrir
 *   no GitHub" fica desabilitado; o diálogo passa no axe sem violação séria, fechado e com os
 *   recolhidos abertos;
 * - CA-UI02-02, CA-UI03-01, CA-UI03-06 e CA-UI03-07: categoria, título e descrição; os passos
 *   do envio à vista; a pré-visualização (recolhida) mostra o que foi escrito e o que o Warden
 *   coletou, e o navegador recebe a página de issue nova do Warden com o título, o corpo exato
 *   da pré-visualização e os rótulos `relato-do-dono` e `categoria: <nome>`;
 * - CA-UI03-04: com "Copiar um print desta tela", o app copia o print da janela (no build de
 *   debug, gravado também em `WARDEN_E2E_CAPTURE_FILE`) antes de abrir o navegador;
 * - CA-UI02-04: um registro com a pasta do usuário, chaves nos formatos da CurseForge, do
 *   Gemini e do GitHub e o nome do computador não deixa nada disso no corpo nem no endereço;
 * - CA-UI02-05: o atalho Ctrl+Shift+E abre o diálogo de outra tela, e Esc fecha;
 * - foco: fechado, o foco volta ao botão (aberto por ele) ou ao que estava focado antes do
 *   atalho (sem nada focado, ao botão);
 * - CA-UI03-07 e CA-UI03-08 pelo teclado: os recolhidos abrem e fecham com Enter e Espaço no
 *   `<summary>`; as setas trocam de categoria (e o corpo da pré-visualização acompanha); o
 *   atalho do cabeçalho tem texto para leitor de tela.
 *
 * O navegador não abre: com `WARDEN_E2E_OPEN_URL` (`wdio.conf.ts`), o app grava nesse arquivo
 * o endereço que abriria.
 */
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { hostname, userInfo } from 'node:os';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';
import { dismissToasts } from './toasts';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const OPENED_FILE = env('WARDEN_E2E_OPEN_URL');
const CAPTURE_FILE = env('WARDEN_E2E_CAPTURE_FILE');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const LOGS_DIR = join(DATA_ROOT, 'data', 'logs');
/** Um registro "do futuro": é o arquivo mais novo da pasta, lido primeiro pelo relato. */
const FAKE_LOG = join(LOGS_DIR, 'warden.9999-12-31.log');
const ISSUE_PREFIX = 'https://github.com/Kriticales/Warden/issues/new?';

/** Valores falsos nos formatos reais, montados em pedaços (nunca uma chave de verdade). */
const FAKE_SECRETS = [
  ['$2a$10$', 'e2eFalsaChaveDaCurseForge0123456789abcdefghijklm'].join(''),
  ['AIza', 'SyE2eFalsaChaveDoGemini0123456789abc'].join(''),
  ['ghp_', 'E2eFalsoTokenDoGitHub0123456789abcdef'].join(''),
];
const USER = userInfo().username;
const COMPUTER = hostname();

/** O que não pode aparecer: segredos e os nomes de 4 letras ou mais (os curtos só como palavra). */
function forbidden(): string[] {
  return [...FAKE_SECRETS, ...[USER, COMPUTER].filter((name) => name.length >= 4)];
}

function leaks(text: string): string[] {
  const lower = text.toLowerCase();
  return forbidden().filter((value) => lower.includes(value.toLowerCase()));
}

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.pause(250);
  await browser.saveScreenshot(join(SCREENSHOTS, `relato-${name}.png`));
}

const dialog = () => $('[role="dialog"]');
const openButton = () => $('[data-testid="report-open"]');

/** Espera a pré-visualização (recolhida) ficar pronta. */
async function waitForPreview(): Promise<void> {
  await browser.waitUntil(async () => (await previewText()) !== '', {
    timeoutMsg: 'a pré-visualização não foi montada',
  });
}

/** Abre um dos recolhidos pelo texto do `<summary>`. */
async function expand(summary: string): Promise<void> {
  const details = $(`summary=${summary}`);
  await details.waitForClickable();
  await details.click();
}

/** Lê o cabeçalho de um PAM (`P7`): largura, altura e quantas cores diferentes há na imagem. */
function readPam(path: string): { width: number; height: number; colors: number } {
  const bytes = readFileSync(path);
  const end = bytes.indexOf('ENDHDR\n');
  const header = bytes.subarray(0, end).toString('latin1');
  const number = (name: string) => Number(new RegExp(`${name} (\\d+)`).exec(header)?.[1] ?? 0);
  const width = number('WIDTH');
  const height = number('HEIGHT');
  const pixels = bytes.subarray(end + 'ENDHDR\n'.length);
  if (pixels.length !== width * height * 4) {
    throw new Error(
      `PAM com ${String(pixels.length)} bytes para ${String(width)}×${String(height)}`,
    );
  }
  const colors = new Set<number>();
  for (let offset = 0; offset < pixels.length && colors.size < 50; offset += 4 * 97) {
    colors.add(pixels.readUInt32BE(offset));
  }
  return { width, height, colors: colors.size };
}

/** Clica em "Abrir no GitHub" e devolve o endereço que o app mandou abrir. */
async function openAndReadUrl(): Promise<string> {
  rmSync(OPENED_FILE, { force: true });
  await openButton().waitForEnabled();
  await openButton().click();
  await browser.waitUntil(() => existsSync(OPENED_FILE), {
    timeoutMsg: 'o app não pediu para abrir o navegador',
  });
  return readFileSync(OPENED_FILE, 'utf8');
}

/** Espera o aviso com este texto. */
async function waitForToast(text: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (wanted) =>
          Array.from(document.querySelectorAll('.toast')).some((toast) =>
            toast.textContent.includes(wanted),
          ),
        text,
      ),
    { timeoutMsg: `o aviso "${text}" não apareceu` },
  );
}

/**
 * Marca o elemento focado agora (`data-e2e-focado`); devolve `false` se nada está focado.
 */
async function markFocused(): Promise<boolean> {
  return browser.execute(() => {
    const active = document.activeElement;
    if (!active || active === document.body) return false;
    active.setAttribute('data-e2e-focado', '');
    return true;
  });
}

/** Quem está focado: `marcado`, o `data-testid` ou o nome da tag (`BODY` se nada). */
async function focused(): Promise<string> {
  return browser.execute(() => {
    const active = document.activeElement;
    if (!active) return 'nenhum';
    if (active.hasAttribute('data-e2e-focado')) return 'marcado';
    return active.getAttribute('data-testid') ?? active.tagName;
  });
}

/** Fecha o diálogo com Esc e espera ele sumir. */
async function closeWithEscape(): Promise<void> {
  await browser.keys(['Escape']);
  await dialog().waitForDisplayed({ reverse: true });
}

/** Põe o foco no `<summary>` com este texto (como o Tab faria, sem depender da ordem). */
async function focusSummary(text: string): Promise<void> {
  const focusedNow = await browser.execute((wanted) => {
    const summary = Array.from(document.querySelectorAll<HTMLElement>('summary')).find(
      (element) => element.textContent === wanted,
    );
    summary?.focus();
    return summary !== undefined && document.activeElement === summary;
  }, text);
  if (!focusedNow) throw new Error(`o resumo "${text}" não recebeu o foco`);
}

/** Se o `<details>` do `<summary>` com este texto está aberto. */
async function isExpanded(text: string): Promise<boolean | null> {
  return browser.execute((wanted) => {
    const summary = Array.from(document.querySelectorAll('summary')).find(
      (element) => element.textContent === wanted,
    );
    return summary?.closest('details')?.open ?? null;
  }, text);
}

/** Espera o `<details>` do `<summary>` com este texto ficar aberto (ou fechado). */
async function waitExpanded(text: string, open: boolean): Promise<void> {
  await browser.waitUntil(async () => (await isExpanded(text)) === open, {
    timeoutMsg: `"${text}" não ficou ${open ? 'aberto' : 'fechado'} pelo teclado`,
  });
}

/** A categoria marcada e se o foco está nela (o rádio). */
async function checkedCategory(): Promise<{ value: string | null; focused: boolean }> {
  return browser.execute(() => {
    const checked = document.querySelector<HTMLInputElement>(
      '[role="dialog"] input[type="radio"]:checked',
    );
    return { value: checked?.value ?? null, focused: checked === document.activeElement };
  });
}

/** Aperta uma seta e espera a categoria e o corpo da pré-visualização acompanharem. */
async function arrowTo(key: 'ArrowLeft' | 'ArrowRight', value: string, name: string) {
  await browser.keys([key]);
  await browser.waitUntil(
    async () => {
      const category = await checkedCategory();
      return category.value === value && category.focused;
    },
    { timeoutMsg: `${key} não marcou a categoria "${value}"` },
  );
  await browser.waitUntil(async () => (await previewText()).startsWith(`## ${name}\n`), {
    timeoutMsg: `a pré-visualização não mudou para "## ${name}"`,
  });
}

/** Texto exato da pré-visualização do corpo (o `<pre>`). */
async function previewText(): Promise<string> {
  return browser.execute(
    () => document.querySelector('[data-testid="report-preview-body"]')?.textContent ?? '',
  );
}

describe('Relatar problema', () => {
  before(async () => {
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    mkdirSync(LOGS_DIR, { recursive: true });
    writeFileSync(
      FAKE_LOG,
      [
        `2026-10-12T10:00:00Z INFO warden: pasta C:\\Users\\${USER}\\AppData\\Roaming\\warden`,
        `2026-10-12T10:00:01Z INFO warden: pasta /home/${USER}/.local/share/warden`,
        `2026-10-12T10:00:02Z DEBUG warden: chave da CurseForge ${FAKE_SECRETS[0] ?? ''}`,
        `2026-10-12T10:00:03Z DEBUG warden: chave do Gemini ${FAKE_SECRETS[1] ?? ''}`,
        `2026-10-12T10:00:04Z DEBUG warden: token do GitHub ${FAKE_SECRETS[2] ?? ''}`,
        `2026-10-12T10:00:05Z INFO warden: computador ${COMPUTER}`,
        '2026-10-12T10:00:06Z INFO warden: última linha do registro de teste',
        '',
      ].join('\n'),
    );
    rmSync(OPENED_FILE, { force: true });
    rmSync(CAPTURE_FILE, { force: true });
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
    await $('h1=Meus packs').waitForDisplayed();
  });

  after(() => {
    // Devolve a pasta de dados como estava: o settings.e2e.ts, que roda depois (ordem
    // alfabética), começa pela primeira execução.
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(FAKE_LOG, { force: true });
    rmSync(OPENED_FILE, { force: true });
    rmSync(CAPTURE_FILE, { force: true });
  });

  it('CA-UI02-01, CA-UI03-06 e CA-UI03-08: o botão abre o diálogo; sem título não abre o GitHub; sem violação séria', async () => {
    const button = $('[data-testid="report-button"]');
    await button.waitForClickable();
    await expect(button).toHaveText('Relatar problema');
    await button.click();
    await dialog().waitForDisplayed();
    await expect($('h2=Relatar problema')).toBeDisplayed();
    await expect(
      $('span=O repositório é público: o relato fica visível para qualquer pessoa.'),
    ).toBeDisplayed();
    await expect($('legend=Categoria')).toBeDisplayed();
    expect(
      await browser.execute(
        () => document.querySelector<HTMLInputElement>('input[type="radio"]:checked')?.value,
      ),
    ).toBe('error');
    const steps = await $('[data-testid="report-steps"]').getText();
    expect(steps).toContain('Abrir no GitHub');
    expect(steps).toContain('Colar os prints');
    expect(steps).toContain('Submit new issue');
    await waitForPreview();
    await expect(openButton()).toBeDisabled();
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('00-aberto');
  });

  it('fechado com Esc ou Cancelar, o foco volta ao botão "Relatar problema"', async () => {
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'o foco não voltou ao botão depois do Esc',
    });
    const button = $('[data-testid="report-button"]');
    await button.click();
    await dialog().waitForDisplayed();
    // O que o Warden coletou chega depois e mexe na altura do diálogo: clicar antes disso pode
    // errar o botão.
    await waitForPreview();
    await $('button=Cancelar').click();
    await dialog().waitForDisplayed({ reverse: true });
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'o foco não voltou ao botão depois do Cancelar',
    });
    // O próximo teste continua com o diálogo aberto.
    await button.click();
    await dialog().waitForDisplayed();
    await waitForPreview();
  });

  it('CA-UI02-02, CA-UI02-04, CA-UI03-01 e CA-UI03-07: categoria e rótulos; a pré-visualização é o que o navegador recebe, sem dados pessoais', async () => {
    await $('span=Visual ou texto').click();
    await $('[data-testid="report-title"]').setValue('E2E: botão cortado na tela de Exportar');
    await $('[data-testid="report-description"]').setValue('O botão aparece cortado pela metade.');
    await browser.waitUntil(async () => (await previewText()).includes('cortado pela metade'), {
      timeoutMsg: 'a pré-visualização não acompanhou o texto',
    });
    await expect($('[data-testid="report-count"]')).toHaveText(
      expect.stringContaining('36 / 4000'),
    );
    await expand('O que o Warden coleta sozinho');
    await expand('Ver como a issue vai ficar');
    const collected = await $('[data-testid="report-collected"]').getText();
    expect(collected).toContain('Meus packs');
    expect(collected).toContain('Warden');
    const body = await previewText();
    expect(body.startsWith('## Visual ou texto\n\nO botão aparece cortado pela metade.')).toBe(
      true,
    );
    expect(body).toContain('última linha do registro de teste');
    expect(body).toContain('<segredo>');
    expect(body).toContain('<usuário>');
    expect(leaks(body)).toEqual([]);
    await expect($('[data-testid="report-preview-title"]')).toHaveText(
      'E2E: botão cortado na tela de Exportar',
    );
    await expect($('[data-testid="report-preview-labels"]')).toHaveText(
      expect.stringContaining('categoria: visual ou texto'),
    );
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('01-preenchido');

    const url = await openAndReadUrl();
    expect(url.startsWith(ISSUE_PREFIX)).toBe(true);
    expect(url.length).toBeLessThanOrEqual(7500);
    expect(url).toContain('&labels=relato-do-dono%2Ccategoria%3A%20visual%20ou%20texto&');
    const params = new URL(url).searchParams;
    expect(params.get('title')).toBe('E2E: botão cortado na tela de Exportar');
    expect(params.get('labels')).toBe('relato-do-dono,categoria: visual ou texto');
    expect(params.get('body')).toBe(body);
    expect(leaks(url)).toEqual([]);
    expect(leaks(decodeURIComponent(url))).toEqual([]);
    // Sem o print marcado, nada de captura.
    expect(existsSync(CAPTURE_FILE)).toBe(false);

    await waitForToast('Relato aberto no navegador');
    await dialog().waitForDisplayed({ reverse: true });
    // O aviso fica no canto de baixo, perto do botão do rodapé que o próximo teste clica.
    await dismissToasts();
  });

  it('CA-UI03-04: com "Copiar um print desta tela", o print da janela sai antes de abrir o navegador', async () => {
    // Sem compositor (Xvfb), o X11 só entrega a parte da janela dentro da tela.
    const size = await browser.execute(() => ({
      width: Math.round(Math.min(window.innerWidth, window.screen.width) * window.devicePixelRatio),
      height: Math.round(
        Math.min(window.innerHeight, window.screen.height) * window.devicePixelRatio,
      ),
    }));
    await $('[data-testid="report-button"]').click();
    await dialog().waitForDisplayed();
    await waitForPreview();
    await $('span=Lentidão').click();
    await $('[data-testid="report-screenshot"] input[type="checkbox"]').click();
    await $('[data-testid="report-title"]').setValue('E2E: abrir o pack demora');
    await browser.waitUntil(async () => (await previewText()).startsWith('## Lentidão'), {
      timeoutMsg: 'a pré-visualização não mudou de categoria',
    });
    const url = await openAndReadUrl();
    expect(new URL(url).searchParams.get('labels')).toBe('relato-do-dono,categoria: lentidão');
    // O print saiu antes do endereço: o arquivo já existe quando o navegador "abre".
    expect(existsSync(CAPTURE_FILE)).toBe(true);
    const shot = readPam(CAPTURE_FILE);
    // Só a janela do Warden: o tamanho é o da janela (com folga para bordas), não o da tela.
    expect(shot.width).toBeGreaterThanOrEqual(size.width - 64);
    expect(shot.height).toBeGreaterThanOrEqual(size.height - 64);
    expect(shot.width).toBeLessThanOrEqual(size.width + 64);
    expect(shot.height).toBeLessThanOrEqual(size.height + 64);
    expect(shot.colors).toBeGreaterThan(1);
    // Com os registros longos dos outros testes, o relato é cortado e o aviso ganha o botão
    // "Copiar relato completo"; os dois começam igual.
    await waitForToast('Cole o print (Ctrl+V) na issue');
    await dialog().waitForDisplayed({ reverse: true });
    // Com o relato cortado, o aviso fica na tela até ser fechado (CA-UI03-05).
    await dismissToasts();
    rmSync(CAPTURE_FILE, { force: true });
  });

  it('CA-UI02-05: o atalho Ctrl+Shift+E abre o diálogo em outra tela, e Esc fecha', async () => {
    // Fechado o diálogo, o foco volta a "Relatar problema" e a dica dele abre. No Windows o
    // botão fica na barra de título e a dica sai para baixo, por cima de Configurações; o Esc
    // esconde a dica, como faria a pessoa.
    await browser.keys(['Escape']);
    await $('.tooltip').waitForExist({ reverse: true });
    const link = $('header.topbar').$('a*=Configurações');
    await link.waitForClickable();
    await link.click();
    await $('h1=Configurações').waitForDisplayed();
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    await expand('O que o Warden coleta sozinho');
    const collected = $('[data-testid="report-collected"]');
    await collected.waitForDisplayed();
    await expect(collected).toHaveText(expect.stringContaining('Configurações'));
    await screenshot('02-atalho');
    await closeWithEscape();
  });

  it('aberto pelo atalho, o foco volta ao que estava focado; sem nada focado, ao botão', async () => {
    const tasks = $('[data-testid="tasks-indicator"]');
    await browser.execute(() => {
      document.querySelector<HTMLElement>('[data-testid="tasks-indicator"]')?.focus();
    });
    expect(await markFocused()).toBe(true);
    await expect(tasks).toBeFocused();
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'marcado', {
      timeoutMsg: 'o foco não voltou ao elemento focado antes do atalho',
    });

    await browser.execute(() => {
      (document.activeElement as HTMLElement | null)?.blur();
    });
    expect(await focused()).toBe('BODY');
    await browser.keys(['Control', 'Shift', 'e']);
    await dialog().waitForDisplayed();
    await closeWithEscape();
    await browser.waitUntil(async () => (await focused()) === 'report-button', {
      timeoutMsg: 'sem nada focado antes, o foco não foi para o botão',
    });
  });

  it('CA-UI03-07 e CA-UI03-08: pelo teclado, os recolhidos abrem com Enter e Espaço e as setas trocam de categoria', async () => {
    await $('[data-testid="report-button"]').click();
    await dialog().waitForDisplayed();
    await waitForPreview();

    // O atalho do cabeçalho: o leitor de tela ouve "Atalho: Ctrl+Shift+E"; quem vê, só a tecla.
    const shortcut = await browser.execute(() => {
      const kbd = document.querySelector('[role="dialog"] kbd');
      const holder = kbd?.parentElement;
      const hidden = holder?.querySelector('.sr-only');
      const box = hidden?.getBoundingClientRect();
      return {
        text: holder?.textContent ?? null,
        hiddenText: hidden?.textContent ?? null,
        hiddenSize: box ? box.width * box.height : null,
        hiddenAria: hidden?.closest('[aria-hidden="true"]') !== null,
      };
    });
    expect(shortcut).toEqual({
      text: 'Atalho: Ctrl+Shift+E',
      hiddenText: 'Atalho: ',
      hiddenSize: 1,
      hiddenAria: false,
    });
    await expect($('[data-testid="report-button"]')).toHaveAttribute(
      'aria-keyshortcuts',
      'Control+Shift+E',
    );

    // Recolhidos: Enter abre e fecha; Espaço também.
    for (const summary of ['O que o Warden coleta sozinho', 'Ver como a issue vai ficar']) {
      expect(await isExpanded(summary)).toBe(false);
      await focusSummary(summary);
      await browser.keys(['Enter']);
      await waitExpanded(summary, true);
      await browser.keys(['Enter']);
      await waitExpanded(summary, false);
      await browser.keys([' ']);
      await waitExpanded(summary, true);
      await browser.keys([' ']);
      await waitExpanded(summary, false);
      // Fica aberto para a próxima parte (e o axe medir os recolhidos abertos).
      await browser.keys([' ']);
      await waitExpanded(summary, true);
    }
    await $('[data-testid="report-collected"]').waitForDisplayed();
    await $('[data-testid="report-preview-body"]').waitForDisplayed();

    // Categoria: com o foco no rádio marcado, as setas andam pelo grupo nos dois sentidos (dar
    // a volta do último para o primeiro depende do motor: o WebView2 dá, o WebKitGTK não).
    expect(await previewText()).toMatch(/^## Erro\n/);
    await browser.execute(() => {
      document
        .querySelector<HTMLInputElement>('[role="dialog"] input[type="radio"]:checked')
        ?.focus();
    });
    expect(await checkedCategory()).toEqual({ value: 'error', focused: true });
    await arrowTo('ArrowRight', 'visual', 'Visual ou texto');
    await arrowTo('ArrowRight', 'slowness', 'Lentidão');
    await arrowTo('ArrowRight', 'suggestion', 'Sugestão');
    await arrowTo('ArrowRight', 'other', 'Outro');
    await expect($('[data-testid="report-preview-labels"]')).toHaveText(
      expect.stringContaining('categoria: outro'),
    );
    await arrowTo('ArrowLeft', 'suggestion', 'Sugestão');
    await arrowTo('ArrowLeft', 'slowness', 'Lentidão');
    await arrowTo('ArrowLeft', 'visual', 'Visual ou texto');
    await arrowTo('ArrowLeft', 'error', 'Erro');
    expect(await seriousViolationSummaries()).toEqual([]);
    await screenshot('03-teclado');
    await closeWithEscape();
  });
});
