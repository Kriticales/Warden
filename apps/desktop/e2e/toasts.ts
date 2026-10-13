/**
 * Ajuda comum dos E2E: fechar os avisos (toasts). Eles ficam fixos no canto de baixo à direita,
 * onde também ficam botões que o teste clica (Exportar…, Voltar para esta versão). Um aviso da
 * etapa anterior (6 s na tela) cobre o botão e o clique é interceptado, sobretudo no Linux da CI,
 * onde as etapas rodam mais depressa. O usuário resolve fechando o aviso ou esperando; o teste
 * fecha antes de clicar.
 */
import { browser } from '@wdio/globals';

export async function dismissToasts(): Promise<void> {
  // Fecha os que estão na tela (e os que chegarem enquanto isso) até não sobrar nenhum: eles
  // saem com uma animação de fade-out (200 ms) e só então deixam a página.
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const toasts = document.querySelectorAll('.toast');
        for (const toast of toasts) {
          if (toast.getAttribute('data-state') !== 'closed') {
            toast.querySelector<HTMLButtonElement>('button:last-child')?.click();
          }
        }
        return toasts.length === 0;
      }),
    { timeout: 5_000, interval: 50, timeoutMsg: 'os avisos não saíram da tela depois de fechados' },
  );
}
