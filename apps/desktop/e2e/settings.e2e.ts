/**
 * Primeira execução e Configurações no app real (P1-13), com o cofre de teste em arquivo e a
 * pasta de dados temporária do `wdio.conf.ts` (nunca o cofre nem as pastas reais):
 *
 * - CA-T01-01: com a pasta de dados vazia, o app abre na primeira execução; ao concluir, abre
 *   no início, e reiniciar não mostra o assistente de novo (o `settings.json` existe);
 * - CA-T01-02: `Zé` é recusado com a regra; `Ze_123` é aceito;
 * - Configurações com o menu lateral (esboço `design/bug-01/configuracoes.html`): cada
 *   página com endereço próprio e a atual marcada;
 * - CA-T21-01 (interface): a chave salva aparece só como `••••xxxx` e continua depois de
 *   reiniciar;
 * - CA-T21-07 e ADR-0069: quem estava no antigo modo `.env` tem a chave levada para o cofre ao
 *   abrir o app, com aviso, e o `.env` some.
 *
 * O "Testar" da CurseForge (CA-T21-02) fica nos testes de componente e de backend: aqui ele
 * falaria com a CurseForge de verdade. O diálogo nativo de pasta também não é aberto (ele
 * pararia o WebDriver); a validação da pasta tem testes no Rust.
 *
 * Roda antes da fumaça (ordem alfabética dos arquivos) e deixa o app configurado.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, $$, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';

const SCREENSHOTS = process.env.WARDEN_E2E_SCREENSHOTS;
/** Valor falso: nunca é uma chave de verdade. */
const FAKE_KEY = 'e2e-chave-falsa-0123456789';

async function screenshot(name: string): Promise<void> {
  if (!SCREENSHOTS) return;
  mkdirSync(SCREENSHOTS, { recursive: true });
  await browser.saveScreenshot(join(SCREENSHOTS, `settings-${name}.png`));
}

/** Reinicia o app (sessão nova do WebDriver = processo novo, mesma pasta de dados). */
async function restartApp(): Promise<void> {
  await browser.reloadSession();
  await $('header.topbar').waitForDisplayed();
}

/** Abre Configurações pela barra do app (cai em Geral) e vai para Chaves e contas. */
async function openSettings(): Promise<void> {
  // Dentro de Configurações a barra não repete o link: volta a Meus packs antes.
  if (!(await $('header.topbar').$('a*=Configurações').isExisting())) {
    await $('header.topbar').$('a*=Meus packs').click();
    await $('h1=Meus packs').waitForDisplayed();
  }
  const link = $('header.topbar').$('a*=Configurações');
  await link.waitForClickable();
  await link.click();
  await $('h1*=Geral').waitForDisplayed();
}

async function openKeys(): Promise<void> {
  await openSettings();
  await $('nav.sidenav').$('a*=Chaves e contas').click();
  await $('h1*=Chaves e contas').waitForDisplayed();
  await $('[data-testid="chave-curseforge"]').waitForDisplayed();
}

const curseforgeRow = () => $('[data-testid="chave-curseforge"]');
/** O texto do item marcado como página atual no menu lateral. */
async function currentPage(): Promise<string> {
  return browser.execute(
    () => document.querySelector('nav.sidenav a[aria-current="page"]')?.textContent ?? '',
  );
}

/** Pasta de configuração do app de teste (`WARDEN_DATA_ROOT/config`). */
function configDir(): string {
  const root = process.env.WARDEN_E2E_DATA_ROOT;
  if (!root) throw new Error('WARDEN_E2E_DATA_ROOT não definida');
  return join(root, 'config');
}

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
    expect(await seriousViolationSummaries()).toEqual([]);
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
    expect(await seriousViolationSummaries()).toEqual([]);
    await $('button=Concluir').click();
    await $('h1=Meus packs').waitForDisplayed();
    await expect($('ol[aria-label="Etapas da primeira execução"]')).not.toBeExisting();
  });

  it('o menu lateral tem as 8 páginas, cada uma com endereço próprio, e marca a atual', async () => {
    await openSettings();
    const names = await browser.execute(() =>
      [...document.querySelectorAll('nav.sidenav .sidenav__name')].map((item) => item.textContent),
    );
    expect(names).toEqual([
      'Geral',
      'Chaves e contas',
      'Sensor Sculk',
      'Teste',
      'Editor de configs',
      'Privacidade e registros',
      'Armazenamento',
      'Sobre o Warden',
    ]);
    expect(await currentPage()).toContain('Geral');
    await screenshot('05-geral');
    expect(await seriousViolationSummaries()).toEqual([]);
    for (const [page, title] of [
      ['sensor-sculk', 'Sensor Sculk'],
      ['teste', 'Teste'],
      ['editor', 'Editor de configs'],
      ['registros', 'Privacidade e registros'],
      ['armazenamento', 'Armazenamento'],
      ['sobre', 'Sobre o Warden'],
    ] as const) {
      await $('nav.sidenav').$(`a[href="/configuracoes/${page}"]`).click();
      await $(`h1*=${title}`).waitForDisplayed();
      expect(await currentPage()).toContain(title);
      await screenshot(`06-${page}`);
      expect(await seriousViolationSummaries()).toEqual([]);
    }
    // Sobre: o commit do packwiz embutido (T23).
    await expect($('dd*=ef87d96')).toBeDisplayed();
  });

  it('CA-T21-01: a chave salva aparece só como ••••final, ainda não testada', async () => {
    await openKeys();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('••••6789'));
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Não testada'));
    await curseforgeRow().click();
    await expect($('section.keydet')).toHaveText(expect.stringContaining('••••6789'));
    await expect($('section.keydet input[type="password"]')).not.toBeExisting();
    await expect($('div.vault')).toHaveText(expect.stringMatching(/[Cc]ofre do (sistema|Windows)/));
    const html = await browser.execute(() => document.documentElement.outerHTML);
    expect(html.includes(FAKE_KEY)).toBe(false);
    await screenshot('07-chaves');
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('CA-T01-01 e CA-T21-01: reiniciar abre no início e a chave continua guardada', async () => {
    await restartApp();
    await $('h1=Meus packs').waitForDisplayed();
    await expect($('ol[aria-label="Etapas da primeira execução"]')).not.toBeExisting();
    await openSettings();
    const name = await $('input.input:not(.input--mono)').getValue();
    expect(name).toBe('Ze_123');
    await $('nav.sidenav').$('a*=Chaves e contas').click();
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('••••6789'));
  });

  it('CA-T21-07 e ADR-0069: o modo .env antigo é levado para o cofre ao abrir, com aviso, e o .env some', async () => {
    const settingsFile = join(configDir(), 'settings.json');
    const settings = JSON.parse(readFileSync(settingsFile, 'utf8')) as Record<string, unknown>;
    writeFileSync(
      settingsFile,
      JSON.stringify({ ...settings, secretsBackend: 'envfile' }, null, 2),
    );
    const envFile = join(configDir(), '.env');
    writeFileSync(envFile, `GITHUB_TOKEN='e2e-token-falso-do-github-4321'\n`);
    await restartApp();
    await $('.toast*=Suas chaves foram para o').waitForDisplayed();
    expect(existsSync(envFile)).toBe(false);
    expect(readFileSync(settingsFile, 'utf8')).not.toContain('secretsBackend');
    await openKeys();
    await expect($('div.alert*=Suas chaves agora ficam no')).toBeDisplayed();
    await expect($('[data-testid="chave-github"]')).toHaveText(expect.stringContaining('••••4321'));
    await screenshot('08-migracao');
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('Remover pede confirmação e a chave volta a ser um campo vazio', async () => {
    await curseforgeRow().click();
    await $('section.keydet').$('button=Remover').click();
    const dialog = $('[role="alertdialog"]');
    await dialog.waitForDisplayed();
    await $('button=Remover chave').click();
    await $('section.keydet input[type="password"]').waitForDisplayed();
    await expect($('section.keydet input[type="password"]')).toHaveValue('');
    await expect(curseforgeRow()).toHaveText(expect.stringContaining('Sem chave'));
  });

  it('em 1024 px nada estoura na horizontal', async () => {
    await browser.setWindowSize(1024, 700);
    await browser.pause(300);
    const overflow = await browser.execute(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBe(0);
    await screenshot('09-configuracoes-1024');
  });
});
