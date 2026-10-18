/** Texto exato numa caixa rolável (`.code--scroll`) que o teclado alcança. */
import type { ReactNode } from 'react';

import { cn } from '../../../lib/cn';

export function ScrollCode({
  label,
  className,
  children,
}: {
  /** Nome da região para o leitor de tela. */
  label?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <pre
      className={cn('code code--scroll', className)}
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
      tabIndex={0}
      aria-label={label}
    >
      {children}
    </pre>
  );
}
