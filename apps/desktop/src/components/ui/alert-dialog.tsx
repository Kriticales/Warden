/**
 * Diálogo de confirmação destrutiva (shadcn `AlertDialog` sobre o Radix; `role="alertdialog"`).
 * Mesma aparência do `Dialog`, mas não fecha com clique fora e não tem ✕: só Esc ou os botões
 * do rodapé (HANDOFF §4). O foco entra no título.
 */
import * as AlertDialogPrimitive from '@radix-ui/react-alert-dialog';
import { useRef, type ReactNode } from 'react';

import { cn } from '../../lib/cn';

export const AlertDialog = AlertDialogPrimitive.Root;
export const AlertDialogTrigger = AlertDialogPrimitive.Trigger;
export const AlertDialogCancel = AlertDialogPrimitive.Cancel;
export const AlertDialogAction = AlertDialogPrimitive.Action;

export interface AlertDialogContentProps {
  title: ReactNode;
  /** O que vai acontecer, ligado por `aria-describedby`. */
  description: ReactNode;
  size?: 'sm' | 'md' | 'lg';
  footer: ReactNode;
  children?: ReactNode;
}

export function AlertDialogContent({
  title,
  description,
  size = 'sm',
  footer,
  children,
}: AlertDialogContentProps) {
  const titleRef = useRef<HTMLHeadingElement>(null);
  return (
    <AlertDialogPrimitive.Portal>
      <AlertDialogPrimitive.Overlay className="scrim">
        <AlertDialogPrimitive.Content
          className={cn('dialog', size !== 'md' && `dialog--${size}`)}
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            titleRef.current?.focus();
          }}
        >
          <div className="dialog__head">
            <div>
              <AlertDialogPrimitive.Title ref={titleRef} className="dialog__title" tabIndex={-1}>
                {title}
              </AlertDialogPrimitive.Title>
              <AlertDialogPrimitive.Description className="dialog__sub">
                {description}
              </AlertDialogPrimitive.Description>
            </div>
          </div>
          {children ? <div className="dialog__body">{children}</div> : null}
          <div className="dialog__foot">{footer}</div>
        </AlertDialogPrimitive.Content>
      </AlertDialogPrimitive.Overlay>
    </AlertDialogPrimitive.Portal>
  );
}
