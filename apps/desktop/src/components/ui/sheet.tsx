/**
 * Painel lateral (shadcn `Sheet side="right"` sobre o Radix Dialog) com a aparência `.drawer`
 * (HANDOFF §3): detalhes do mod, Tarefas. O foco entra no título, Tab fica preso, Esc ou ✕
 * fecham e o foco volta para quem abriu.
 */
import * as DialogPrimitive from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import { useRef, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from './button';

export const Sheet = DialogPrimitive.Root;
export const SheetTrigger = DialogPrimitive.Trigger;

export interface SheetContentProps {
  title: ReactNode;
  footer?: ReactNode;
  children?: ReactNode;
}

export function SheetContent({ title, footer, children }: SheetContentProps) {
  const { t } = useTranslation();
  const titleRef = useRef<HTMLHeadingElement>(null);
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="scrim scrim--drawer">
        <DialogPrimitive.Content
          className="drawer"
          aria-describedby={undefined}
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            titleRef.current?.focus();
          }}
        >
          <div className="drawer__head">
            <DialogPrimitive.Title ref={titleRef} className="t-title" tabIndex={-1}>
              {title}
            </DialogPrimitive.Title>
            <DialogPrimitive.Close asChild>
              <Button variant="ghost" size="sm" iconOnly icon={X}>
                {t('acoes.fecharPainel')}
              </Button>
            </DialogPrimitive.Close>
          </div>
          <div className="drawer__body">{children}</div>
          {footer ? <div className="drawer__foot">{footer}</div> : null}
        </DialogPrimitive.Content>
      </DialogPrimitive.Overlay>
    </DialogPrimitive.Portal>
  );
}
