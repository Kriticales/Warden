/**
 * Junta classes condicionais (`cn('btn', primary && 'btn--primary')`).
 *
 * Diferente do `cn` do shadcn/ui, não usa `tailwind-merge`: o tema do Warden zera as escalas
 * padrão do Tailwind e tem nomes próprios (`text-text-2`, `bg-surface-1`), que o
 * `tailwind-merge` não conhece e poderia descartar. Os componentes usam as classes do design
 * system (`components.css`) e utilitários só para layout, sem conflito a resolver.
 */
import { clsx, type ClassValue } from 'clsx';

export function cn(...inputs: ClassValue[]): string {
  return clsx(inputs);
}
