/**
 * Ajuda comum dos E2E: trocar o texto de um campo controlado pelo React. O `setValue` do
 * WebDriver limpa o campo sem disparar o `input` que o React escuta; o estado continua com o
 * texto antigo, e o digitado é somado a ele ("1.0.1" + "0.9.0" = "1.0.10.9.0"), conforme o
 * tempo da máquina. Aqui o campo é apagado pelo teclado (como a pessoa faria), o teste espera
 * ele ficar vazio e confere o valor final.
 */
import { browser } from '@wdio/globals';
import type { ChainablePromiseElement } from 'webdriverio';

export async function replaceValue(field: ChainablePromiseElement, text: string): Promise<void> {
  await field.waitForEnabled();
  await field.click();
  await browser.waitUntil(
    async () => {
      if ((await field.getValue()) === '') return true;
      await browser.keys(['Control', 'a']);
      await browser.keys('Backspace');
      return (await field.getValue()) === '';
    },
    { timeout: 10_000, interval: 100, timeoutMsg: 'o campo não ficou vazio' },
  );
  await field.addValue(text);
  await browser.waitUntil(async () => (await field.getValue()) === text, {
    timeout: 10_000,
    timeoutMsg: `o campo não ficou com "${text}"`,
  });
}
