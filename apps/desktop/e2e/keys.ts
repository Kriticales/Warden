/**
 * Salvar uma chave pela tela de Configurações → Chaves e contas (menu lateral; ADR-0069): a
 * chave escolhida na lista, o campo de senha no detalhe e **Salvar**. Depois de salvar, o
 * Warden testa a chave na hora; aqui se espera o teste terminar (funcionando, recusada ou "não
 * deu para testar", conforme o simulado de cada API).
 */
import { $, browser } from '@wdio/globals';

import { dismissToasts } from './toasts';

export type E2eKeyKind = 'curseforge' | 'github' | 'gemini' | 'openai' | 'anthropic';

/** Abre Configurações pela barra do app e vai para Chaves e contas. */
export async function openKeysPage(): Promise<void> {
  await dismissToasts();
  const link = $('header.topbar').$('a*=Configurações');
  await link.waitForClickable({ timeout: 30_000 });
  await link.click();
  await $('h1*=Geral').waitForDisplayed({ timeout: 30_000 });
  await $('nav.sidenav').$('a*=Chaves e contas').click();
  await $('h1*=Chaves e contas').waitForDisplayed({ timeout: 30_000 });
}

/** O item da chave na lista da esquerda. */
export const keyItem = (kind: E2eKeyKind) => $(`[data-testid="chave-${kind}"]`);

/** Salva a chave (já em Chaves e contas) e espera o teste automático terminar. */
export async function saveKey(kind: E2eKeyKind, value: string): Promise<void> {
  const item = keyItem(kind);
  await item.waitForDisplayed({ timeout: 30_000 });
  await item.click();
  const field = $('section.keydet').$('input[type="password"]');
  await field.waitForDisplayed({ timeout: 30_000 });
  await field.setValue(value);
  await $('section.keydet').$('button=Salvar').click();
  await browser.waitUntil(
    async () => {
      const text = await item.getText();
      return text.includes('••••') && !text.includes('Testando');
    },
    { timeout: 60_000, timeoutMsg: `a chave ${kind} não foi salva e testada` },
  );
}

/** Nome do provedor na tela. */
const PROVIDER_NAMES = { gemini: 'Gemini', openai: 'OpenAI', anthropic: 'Claude' } as const;

/**
 * Configurações → Sensor Sculk: põe o provedor em primeiro na ordem (pelas setas) e espera ele
 * abrir as conversas novas (a chave dele precisa estar no cofre; ADR-0070).
 */
export async function useProviderFirst(provider: keyof typeof PROVIDER_NAMES): Promise<void> {
  await dismissToasts();
  const link = $('header.topbar').$('a*=Configurações');
  await link.waitForClickable({ timeout: 30_000 });
  await link.click();
  await $('h1*=Geral').waitForDisplayed({ timeout: 30_000 });
  await $('nav.sidenav').$('a*=Sensor Sculk').click();
  await $('h1*=Sensor Sculk').waitForDisplayed({ timeout: 30_000 });
  const row = $(`.porder__row[data-provider="${provider}"]`);
  await row.waitForDisplayed({ timeout: 30_000 });
  let tries = Object.keys(PROVIDER_NAMES).length;
  while (tries > 0 && (await $('.porder__row').getAttribute('data-provider')) !== provider) {
    tries -= 1;
    const up = row.$(`button[aria-label="Subir ${PROVIDER_NAMES[provider]}"]`);
    await up.waitForEnabled({ timeout: 30_000 });
    await up.click();
    await browser.waitUntil(async () => !(await $('[aria-busy="true"]').isExisting()), {
      timeout: 30_000,
      timeoutMsg: 'a ordem não foi gravada',
    });
  }
  await row.$('span*=Conversas novas').waitForDisplayed({ timeout: 30_000 });
}
