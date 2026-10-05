import type { LucideIcon, LucideProps } from 'lucide-react';

import { cn } from '../../lib/cn';

/** Tamanho do ícone: 14, 16 (padrão) ou 20 px (DESIGN-SYSTEM §3.7). */
export type IconSize = 'sm' | 'md' | 'lg';

const SIZE_CLASS: Record<IconSize, string | undefined> = {
  sm: 'icon--sm',
  md: undefined,
  lg: 'icon--lg',
};

const SIZE_PX: Record<IconSize, number> = { sm: 14, md: 16, lg: 20 };

export interface IconProps extends Omit<LucideProps, 'ref' | 'size'> {
  /** Ícone do `lucide-react`. */
  icon: LucideIcon;
  size?: IconSize;
}

/**
 * Ícone Lucide com a classe `.icon` do design system (traço 2 px). Decorativo
 * (`aria-hidden`): quem precisa de nome acessível é o botão ou o texto ao lado.
 */
export function Icon({ icon: Component, size = 'md', className, ...props }: IconProps) {
  return (
    <Component
      aria-hidden="true"
      focusable="false"
      size={SIZE_PX[size]}
      strokeWidth={2}
      className={cn('icon', SIZE_CLASS[size], className)}
      {...props}
    />
  );
}
