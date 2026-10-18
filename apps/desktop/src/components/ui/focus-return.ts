/**
 * Foco devolvido a quem abriu uma camada (diálogo, confirmação e painel lateral; HANDOFF §7;
 * SPEC §7.6).
 *
 * O Radix só devolve o foco ao `Trigger` da camada. Aberta por estado (clique numa linha, atalho,
 * item de menu), ela deixaria o foco no `<body>` ao fechar. Por isso cada camada guarda, ao abrir,
 * o elemento que estava focado e o foca de novo ao fechar. Item de menu some junto com o menu:
 * quem abriu, então, é o gatilho do menu (o ⋯ da linha). Se nada estava focado, ou o elemento
 * saiu da tela, vale `returnFocusTo` e, sem ele, o padrão do Radix.
 */
import { useRef } from 'react';

/** O elemento focado agora, se for um elemento de verdade (não o `<body>`). */
function focusedElement(): HTMLElement | null {
  const active = document.activeElement;
  return active instanceof HTMLElement && active !== document.body ? active : null;
}

/**
 * Quem abriu a camada: o elemento focado ou, se ele é item de um menu, o gatilho do menu (o
 * conteúdo do menu do Radix é rotulado pelo gatilho, `aria-labelledby`).
 */
function openerOf(element: HTMLElement | null): HTMLElement | null {
  const menu = element?.closest('[role="menu"]');
  const triggerId = menu?.getAttribute('aria-labelledby');
  if (!triggerId) {
    return element;
  }
  return document.getElementById(triggerId) ?? element;
}

export interface FocusReturn {
  /** Chamar no `onOpenAutoFocus`, quando o foco ainda está em quem abriu. */
  rememberOpener: () => void;
  /** O `onCloseAutoFocus` da camada. */
  onCloseAutoFocus: (event: Event) => void;
}

export function useFocusReturn(returnFocusTo?: () => HTMLElement | null): FocusReturn {
  const openerRef = useRef<HTMLElement | null>(null);
  return {
    rememberOpener: () => {
      openerRef.current = openerOf(focusedElement());
    },
    onCloseAutoFocus: (event) => {
      const opener = openerRef.current;
      const target = opener?.isConnected ? opener : (returnFocusTo?.() ?? null);
      if (target) {
        event.preventDefault();
        target.focus();
      }
    },
  };
}
