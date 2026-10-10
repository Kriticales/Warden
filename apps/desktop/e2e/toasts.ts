/**
 * Ajuda comum dos E2E: fechar os avisos (toasts). Eles ficam fixos no canto de baixo à direita,
 * onde também ficam botões que o teste clica (Exportar…, Voltar para esta versão). Um aviso da
 * etapa anterior (6 s na tela) cobre o botão e o clique é interceptado, sobretudo no Linux da CI,
 * onde as etapas rodam mais depressa. O usuário resolve fechando o aviso ou esperando; o teste
 * fecha antes de clicar.
 */
import { browser } from '@wdio/globals';

export async function dismissToasts(): Promise<void> {
  await browser.execute(() => {
    for (const close of document.querySelectorAll<HTMLButtonElement>('.toast button:last-child')) {
      close.click();
    }
  });
  // Os avisos saem com uma animação de fade-out (200 ms).
  await browser.pause(400);
}
