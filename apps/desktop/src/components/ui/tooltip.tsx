/**
 * Tooltip (shadcn `Tooltip` sobre o Radix) com a aparência `.tooltip` do design system.
 * Abre no hover depois de ~400 ms e no foco do teclado; Esc esconde (HANDOFF §3).
 */
import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import type { ReactNode } from 'react';

/** Atraso de abertura (HANDOFF §3). */
export const TOOLTIP_DELAY_MS = 400;

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <TooltipPrimitive.Provider delayDuration={TOOLTIP_DELAY_MS}>
      {children}
    </TooltipPrimitive.Provider>
  );
}

export interface TooltipProps {
  /** Texto da dica. */
  content: ReactNode;
  /** O elemento que recebe a dica (um único elemento focável). */
  children: ReactNode;
  side?: 'top' | 'right' | 'bottom' | 'left';
}

export function Tooltip({ content, children, side = 'top' }: TooltipProps) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content className="tooltip" side={side} sideOffset={6}>
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
