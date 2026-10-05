/**
 * Foco na troca de tela e no atalho "Pular para o conteúdo" (HANDOFF §7): as rotas não usam o
 * hash, então o foco é movido por script.
 */

/** Id do `<main>` de toda tela. */
export const MAIN_ID = 'conteudo';

/** Marca o título que recebe o foco quando a tela muda. */
export const PAGE_TITLE_ATTRIBUTE = 'data-page-title';

/** Move o foco para o conteúdo principal. Devolve se achou. */
export function focusMain(doc: Document = document): boolean {
  const main = doc.getElementById(MAIN_ID);
  if (!main) {
    return false;
  }
  main.focus();
  return true;
}

/**
 * Move o foco para o título da página (`[data-page-title]`, com `tabIndex={-1}`), para o
 * leitor de tela anunciar onde se está; sem título, vai para o `<main>`.
 */
export function focusPageTitle(doc: Document = document): boolean {
  const title = doc.querySelector<HTMLElement>(`[${PAGE_TITLE_ATTRIBUTE}]`);
  if (title) {
    title.focus({ preventScroll: true });
    return true;
  }
  return focusMain(doc);
}
