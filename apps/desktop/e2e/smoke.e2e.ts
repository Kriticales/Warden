/**
 * Fumaça (F0-06, critério 3): o app real abre com a moldura (barra do app, conteúdo, rodapé),
 * "Sobre o Warden" mostra a versão de `app_info` e o aviso legal, a gaveta de Tarefas abre pelo
 * rodapé (com o foco no título) e fecha com Esc devolvendo o foco ao indicador, e o axe não
 * acha violação séria nem crítica no WebView real (aqui o contraste é medido de verdade).
 *
 * Com `WARDEN_E2E_SCREENSHOTS=<pasta>`, grava capturas da janela nessa pasta.
 */
import { mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');
const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.saveScreenshot(join(SCREENSHOTS, `${name}.png`));
}

interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
  nodes: { target: string[] }[];
}

/** Roda o axe-core dentro do WebView e devolve as violações sérias e críticas. */
async function seriousViolations(): Promise<AxeViolation[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async () => {
    const { axe } = window as unknown as {
      axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
    };
    try {
      const result = await axe.run({ resultTypes: ['violations'] });
      return result.violations;
    } catch {
      const failed: AxeViolation = {
        id: 'axe-falhou',
        impact: 'critical',
        help: 'o axe não rodou',
        nodes: [],
      };
      return [failed];
    }
  });
  return violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
}

describe('fumaça', () => {
  it('abre o app com a moldura, a versão e o aviso legal', async () => {
    // O título tem "Warden" num <span> aceso: o texto completo vem do elemento inteiro.
    const title = $('h1*=Boas-vindas ao');
    await title.waitForDisplayed();
    await expect(title).toHaveText('Boas-vindas ao Warden');
    await expect($('header.topbar')).toBeDisplayed();
    await expect($('main#conteudo')).toBeDisplayed();
    const indicator = $('[data-testid="tasks-indicator"]');
    await expect(indicator).toHaveText('Nenhuma tarefa em andamento');
    const legal = $('[data-testid="aviso-legal"]');
    await expect(legal).toHaveText(
      'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.',
    );
    await expect($('footer.statusbar')).toHaveText(expect.stringContaining('Warden 0.1.0'));
    await screenshot('01-inicio');
  });

  it('abre e fecha a gaveta de Tarefas pelo rodapé, pelo teclado', async () => {
    const indicator = $('[data-testid="tasks-indicator"]');
    await indicator.click();
    const drawer = $('.drawer');
    await drawer.waitForDisplayed();
    await expect($('.drawer h2')).toHaveText('Tarefas');
    await expect($('.drawer h2')).toBeFocused();
    await expect(drawer).toHaveText(expect.stringContaining('Nenhuma tarefa ainda'));
    await screenshot('02-tarefas');
    await browser.keys('Escape');
    await drawer.waitForDisplayed({ reverse: true });
    await expect(indicator).toBeFocused();
  });

  it('mostra "Sobre o Warden" com a versão e o commit', async () => {
    const about = $('h2=Sobre o Warden');
    await about.scrollIntoView();
    await expect($('dl.kv')).toHaveText(expect.stringContaining('0.1.0'));
    await screenshot('03-sobre');
  });

  it('em 1024 px (largura mínima do HANDOFF §5) nada estoura na horizontal', async () => {
    await browser.setWindowSize(1024, 700);
    await browser.pause(300);
    const overflow = await browser.execute(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBe(0);
    await $('h1*=Boas-vindas ao').scrollIntoView();
    await screenshot('04-inicio-1024');
    await $('[data-testid="tasks-indicator"]').click();
    await $('.drawer').waitForDisplayed();
    await screenshot('05-tarefas-1024');
    await browser.keys('Escape');
    await $('.drawer').waitForDisplayed({ reverse: true });
  });

  it('não tem violação séria nem crítica do axe (com contraste real)', async () => {
    const violations = await seriousViolations();
    expect(violations.map((violation) => `${violation.id}: ${violation.help}`)).toEqual([]);
  });
});
