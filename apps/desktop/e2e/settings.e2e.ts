/**
 * Primeira execução e Configurações no app real (P1-13), com o cofre de teste em arquivo e a
 * pasta de dados temporária do `wdio.conf.ts` (nunca o cofre nem as pastas reais):
 *
 * - CA-T01-01: com a pasta de dados vazia, o app abre na primeira execução; ao concluir, abre
 *   no início, e reiniciar não mostra o assistente de novo (o `settings.json` existe);
 * - CA-T01-02: `Zé` é recusado com a regra; `Ze_123` é aceito;
 * - CA-T21-01 (interface): a chave salva continua "Configurada" depois de reiniciar;
 * - CA-T21-03 (interface): ir para o `.env` pede confirmação e deixa o aviso fixo, que
 *   continua depois de reiniciar; "Voltar para o cofre" tira o aviso e a chave continua
 *   "Configurada".
 *
 * O "Testar" da CurseForge (CA-T21-02) fica nos testes de componente e de backend: aqui ele
 * falaria com a CurseForge de verdade. O diálogo nativo de pasta também não é aberto (ele
 * pararia o WebDriver); a validação da pasta tem testes no Rust.
 *
 * Roda antes da fumaça (ordem alfabética dos arquivos) e deixa o app configurado.
 */
import { mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');
const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
/** Valor falso: nunca é uma chave de verdade. */
const FAKE_KEY = 'e2e-chave-falsa-0123456789';

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.saveScreenshot(join(SCREENSHOTS, `settings-${name}.png`));
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
    .filter((violation) => violation.impact === 'serious' || violation.impact === 'critical')
    .map((violation) => `${violation.id}: ${violation.help}`);
}

/** Reinicia o app (sessão nova do WebDriver = processo novo, mesma pasta de dados). */
async function restartApp(): Promise<void> {
  await browser.reloadSession();
  await $('header.topbar').waitForDisplayed();
}

async function openSettings(): Promise<void> {
  const link = $('header.topbar').$('a*=Configurações');
  await link.waitForClickable();
  await link.click();
  await $('h1=Configurações').waitForDisplayed();
  await $('[data-testid="chave-curseforge"]').waitForDisplayed();
}

/** Valor do campo ligado ao rótulo com este texto. */
async function fieldValue(label: string): Promise<string | null> {
  return browser.execute((text) => {
    const found = [...document.querySelectorAll('label')].find(
      (element) => element.textContent === text,
    );
    return found?.control instanceof HTMLInputElement ? found.control.value : null;
  }, label);
}

const curseforgeRow = () => $('[data-testid="chave-curseforge"]');
const envWarning = () => $('div.alert*=As chaves estão num arquivo de texto.');

describe('primeira execução e Configurações', () => {
  it('CA-T01-01: com a pasta de dados vazia, abre na primeira execução com o aviso legal', async () => {
    await $('ol[aria-label="Etapas da primeira execução"]').waitForDisplayed();
    await expect($('h1*=Boas-vindas ao')).toHaveText('Boas-vindas ao Warden');
    await expect($('[data-testid="aviso-legal"]')).toHaveText(
      'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.',
    );
    // Sem menu: a barra não tem o link de Configurações.
    await expect($('header.topbar').$('a*=Configurações')).not.toBeExisting();
    await screenshot('01-aviso');
    expect(await seriousViolations()).toEqual([]);
    await $('button=Entendi').click();
  });

  it('CA-T01-02: "Zé" é recusado com a regra; "Ze_123" é aceito', async () => {
    const field = $('.wizard input.input');
    await field.waitForDisplayed();
    await expect($('h1=Nome do jogador nos testes')).toBeFocused();
    await field.clearValue();
    await field.setValue('Zé');
    const error = $('.field__error');
    await expect(error).toHaveText(
      'Use só letras sem acento, números e _ (sem espaço), de 3 a 16 caracteres.',
    );
    await expect($('button=Próximo')).toBeDisabled();
    await screenshot('02-nome-recusado');
    await field.clearValue();
    await field.setValue('Ze_123');
    await expect(error).not.toBeExisting();
    await $('button=Próximo').click();
  });

  it('mostra a pasta padrão dos packs e segue para as chaves', async () => {
    await $('h1=Pasta dos packs').waitForDisplayed();
    const folder = $('input.input--mono');
    await expect(folder).toHaveValue(expect.stringContaining('packs'));
    await screenshot('03-pasta');
    await $('button=Próximo').click();
    await $('h1=Chaves').waitForDisplayed();
  });

  it('CA-T01-01: concluir com uma chave grava tudo e abre o início', async () => {
    const fields = $$('.wizard input[type="password"]');
    await fields[0]?.waitForDisplayed();
    await fields[0]?.setValue(FAKE_KEY);
    await screenshot('04-chaves');
    expect(await seriousViolations()).toEqual([]);
    await $('button=Concluir').click();
    await $('h1=Meus packs').waitForDisplayed();
    await expect($('ol[aria-label="Etapas da primeira execução"]')).not.toBeExisting();
  });

  it('CA-T21-01: a chave salva aparece só como "Configurada"', async () => {
    await openSettings();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Configurada'));
    await expect(curseforgeRow().$('input')).not.toBeExisting();
    const html = await browser.execute(() => document.documentElement.outerHTML);
    expect(html.includes(FAKE_KEY)).toBe(false);
    await screenshot('05-configuracoes');
    expect(await seriousViolations()).toEqual([]);
  });

  it('CA-T01-01 e CA-T21-01: reiniciar abre no início e a chave continua "Configurada"', async () => {
    await restartApp();
    await $('h1=Meus packs').waitForDisplayed();
    await expect($('ol[aria-label="Etapas da primeira execução"]')).not.toBeExisting();
    await openSettings();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Configurada'));
    expect(await fieldValue('Nome do jogador nos testes')).toBe('Ze_123');
  });

  it('CA-T21-03: ir para o .env pede confirmação, e o aviso fica fixo mesmo depois de reiniciar', async () => {
    await $('label.choice*=Arquivo .env').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(
      expect.stringContaining(
        'Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler as chaves.',
      ),
    );
    await screenshot('06-confirmar-env');
    await $('button=Usar arquivo .env').click();
    await envWarning().waitForDisplayed();
    await screenshot('07-env');

    await restartApp();
    await openSettings();
    await envWarning().waitForDisplayed();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Configurada'));
  });

  it('CA-T21-03: "Voltar para o cofre" tira o aviso e a chave continua "Configurada"', async () => {
    await $('button=Voltar para o cofre').click();
    await envWarning().waitForDisplayed({ reverse: true });
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Configurada'));
    await restartApp();
    await openSettings();
    await expect(envWarning()).not.toBeExisting();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Configurada'));
  });

  it('Remover pede confirmação e a chave volta a ser um campo vazio', async () => {
    await curseforgeRow().$('button=Remover').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await $('button=Remover chave').click();
    await curseforgeRow().$('input[type="password"]').waitForDisplayed();
    await expect(curseforgeRow().$('input[type="password"]')).toHaveValue('');
  });

  it('em 1024 px nada estoura na horizontal', async () => {
    await browser.setWindowSize(1024, 700);
    await browser.pause(300);
    const overflow = await browser.execute(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBe(0);
    await screenshot('08-configuracoes-1024');
  });
});
