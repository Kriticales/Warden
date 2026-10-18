/**
 * Ajuda comum dos E2E: os passos que mais de um spec faz do mesmo jeito (publicar, parar o jogo,
 * exportar), tirados de `publish.e2e.ts`, `capture.e2e.ts` e `export.e2e.ts` para o fluxo
 * completo (`full-flow.e2e.ts`, A-03) usar os mesmos, sem cópia. Nenhum espera com pausa fixa:
 * cada um espera a condição real na tela.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { replaceValue } from './fields';
import { dismissToasts } from './toasts';

/** Um diálogo pelo nome acessível (o título), quando ele aparece com o conteúdo pronto. */
export async function dialogNamed(title: string, ready: string, timeout = 60_000) {
  const dialog = $(`[role="dialog"]*=${ready}`);
  await browser.waitUntil(
    async () =>
      (await dialog.isExisting()) &&
      (await dialog.getText()).includes(title) &&
      (await dialog.getText()).includes(ready),
    { timeout, timeoutMsg: `diálogo "${title}" com "${ready}" não apareceu` },
  );
  return dialog;
}

/** Abre uma seção do pack pela navegação lateral e espera o título dela. */
export async function openSection(name: string, heading: string): Promise<void> {
  await dismissToasts();
  await $('nav[aria-label="Seções do pack"]').$(`a*=${name}`).click();
  await $(`h1=${heading}`).waitForDisplayed({ timeout: 30_000 });
}

/** **Salvar versão** pelo cabeçalho do pack, marcada como versão final. */
export async function saveFinal(version: string): Promise<void> {
  await dismissToasts();
  await $('header.packhead').$('button*=Salvar versão').click();
  const dialog = $('[role="dialog"]*=Número da versão');
  await dialog.waitForDisplayed({ timeout: 60_000 });
  const number = dialog.$('input.input');
  await replaceValue(number, version);
  await dialog.$('label*=Marcar como versão final').click();
  const save = dialog.$(`button*=Salvar versão ${version}`);
  await save.waitForEnabled({ timeout: 30_000 });
  await save.click();
  await $(`[role="dialog"]*=Versão ${version} salva como versão final`).waitForDisplayed({
    timeout: 60_000,
  });
}

/** **Parar jogo** na tela do teste, confirmado, até o resultado "Encerrado por você". */
export async function stopGame(): Promise<void> {
  await dismissToasts();
  await $('main').$('button=Parar jogo').click();
  const dialog = $('[role="alertdialog"]');
  await dialog.waitForDisplayed();
  await dialog.$('button=Parar jogo').click();
  await $('h1*=Encerrado por você').waitForDisplayed({ timeout: 30_000 });
}

/** Espera um aviso (toast) com o texto; um aviso de falha encerra a espera e reprova. */
export async function waitToast(text: string): Promise<void> {
  let seen = '';
  await browser.waitUntil(
    async () => {
      seen = await browser.execute(() =>
        Array.from(document.querySelectorAll('.toast'))
          .map((toast) => toast.textContent)
          .join(' | '),
      );
      return seen.includes(text) || seen.includes('Não foi possível') || /falhou/i.test(seen);
    },
    { timeout: 60_000, timeoutMsg: `aviso "${text}" não apareceu (avisos: ${seen})` },
  );
  expect(seen).toContain(text);
}

/** **Exportar…** na seção Exportar, até o diálogo do resultado (um erro na tela reprova). */
export async function exportAndWait(): Promise<void> {
  await dismissToasts();
  await $('button=Exportar…').click();
  const dialog = $('[role="dialog"]*=Pack exportado');
  let error = '';
  await browser.waitUntil(
    async () => {
      if (await dialog.isExisting()) return true;
      const panel = $('main .alert--danger');
      if (await panel.isExisting()) error = await panel.getText();
      return error !== '';
    },
    { timeout: 120_000, timeoutMsg: 'a exportação não terminou' },
  );
  expect(error).toBe('');
}

/** Fecha o diálogo aberto pelo **Fechar** e espera ele sair. */
export async function closeDialog(): Promise<void> {
  await $('[role="dialog"]').$('button=Fechar').click();
  await $('[role="dialog"]').waitForExist({ reverse: true });
}

/** Os arquivos de uma pasta (caminhos com `/`), em ordem. */
export function listFiles(dir: string): string[] {
  const out: string[] = [];
  const walk = (current: string) => {
    for (const entry of readdirSync(current)) {
      const path = join(current, entry);
      if (statSync(path).isDirectory()) walk(path);
      else out.push(relative(dir, path).replaceAll('\\', '/'));
    }
  };
  walk(dir);
  return out.sort();
}

/** `pack.toml`, `index.toml` e os `file = "…"` do índice do pack. */
export function expectedFiles(pack: string): string[] {
  const index = readFileSync(join(pack, 'index.toml'), 'utf8');
  const files = [...index.matchAll(/^file = "([^"]+)"/gm)].map((match) => match[1] ?? '');
  return ['index.toml', 'pack.toml', ...files].sort();
}
