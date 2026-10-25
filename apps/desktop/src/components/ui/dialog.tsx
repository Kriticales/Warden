/**
 * Diálogo (shadcn `Dialog` sobre o Radix) com a aparência `.scrim`/`.dialog` (HANDOFF §3 e §4).
 *
 * - O foco entra no título (`tabIndex={-1}`), para o leitor de tela anunciar o diálogo e o Tab
 *   seguir a ordem do conteúdo; Tab fica preso; Esc, ✕ ou clique fora fecham.
 * - Ao fechar, o foco volta para quem abriu (`focus-return.ts`): o elemento que estava focado
 *   quando o diálogo abriu ou, se era um item de menu, o gatilho do menu. Se nada estava focado
 *   (ou o elemento saiu da tela), vale `returnFocusTo` e, sem ele, o padrão do Radix.
 * - Destrutivo usa `AlertDialog` (`alert-dialog.tsx`), que não fecha com clique fora.
 */
import * as DialogPrimitive from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import { useRef, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '../../lib/cn';
import { Button } from './button';
import { useFocusReturn } from './focus-return';

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export interface DialogContentProps {
  /** Título (Manrope 800; frase curta). */
  title: ReactNode;
  /** Linha abaixo do título, ligada por `aria-describedby`. */
  description?: ReactNode;
  /** Largura: 480, 600 (padrão), 780 ou 1200 px (duas colunas, como o diálogo Adicionar itens). */
  size?: 'sm' | 'md' | 'lg' | 'xl';
  /** Botões do rodapé. */
  footer?: ReactNode;
  /** Rodapé com os botões nas pontas (ação secundária à esquerda). */
  footerSplit?: boolean;
  /** Para onde o foco volta ao fechar quando nada estava focado ao abrir. */
  returnFocusTo?: () => HTMLElement | null;
  /** Conteúdo à direita do título, antes do ✕ (um atalho, por exemplo). */
  aside?: ReactNode;
  /** Classe extra do fundo escurecido (`.scrim`), que contém o diálogo. */
  overlayClassName?: string;
  children?: ReactNode;
}

export function DialogContent({
  title,
  description,
  size = 'md',
  footer,
  footerSplit = false,
  returnFocusTo,
  aside,
  overlayClassName,
  children,
}: DialogContentProps) {
  const { t } = useTranslation();
  const titleRef = useRef<HTMLHeadingElement>(null);
  const focusReturn = useFocusReturn(returnFocusTo);
  const close = (
    <DialogPrimitive.Close asChild>
      <Button variant="ghost" size="sm" iconOnly icon={X}>
        {t('acoes.fechar')}
      </Button>
    </DialogPrimitive.Close>
  );
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className={cn('scrim', overlayClassName)}>
        <DialogPrimitive.Content
          className={cn('dialog', size !== 'md' && `dialog--${size}`)}
          // Sem descrição, o Radix pede `aria-describedby` explícito (vazio).
          {...(description ? {} : { 'aria-describedby': undefined })}
          onOpenAutoFocus={(event) => {
            // O foco ainda está em quem abriu.
            focusReturn.rememberOpener();
            event.preventDefault();
            titleRef.current?.focus();
          }}
          onCloseAutoFocus={focusReturn.onCloseAutoFocus}
        >
          <div className="dialog__head">
            <div>
              <DialogPrimitive.Title ref={titleRef} className="dialog__title" tabIndex={-1}>
                {title}
              </DialogPrimitive.Title>
              {description ? (
                <DialogPrimitive.Description className="dialog__sub">
                  {description}
                </DialogPrimitive.Description>
              ) : null}
            </div>
            {aside ? (
              <div className="row">
                {aside}
                {close}
              </div>
            ) : (
              close
            )}
          </div>
          <div className="dialog__body">{children}</div>
          {footer ? (
            <div className={cn('dialog__foot', footerSplit && 'dialog__foot--split')}>{footer}</div>
          ) : null}
        </DialogPrimitive.Content>
      </DialogPrimitive.Overlay>
    </DialogPrimitive.Portal>
  );
}
