/**
 * Estado vazio (HANDOFF §3; DESIGN-SYSTEM §6): o que é, por que está vazio e o que fazer.
 * Variantes: padrão (tracejado), `ok` (nada de errado), `error` (falha ao carregar) e
 * `compact` (dentro de painéis e gavetas). Ilustração em pixel com um símbolo.
 */
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { EmptyArt, type ArtTone, type Glyph } from './PixelArt';

export type EmptyKind = 'default' | 'ok' | 'error';

export interface EmptyStateProps {
  title: ReactNode;
  text?: ReactNode;
  /** Botões (a ação que resolve). */
  actions?: ReactNode;
  /** Linha pequena depois das ações. */
  hint?: ReactNode;
  kind?: EmptyKind;
  glyph?: Glyph;
  compact?: boolean;
  /** Nível do título, para seguir a hierarquia da página. Padrão: 2. */
  headingLevel?: 2 | 3 | 4;
  className?: string | undefined;
}

const TONE: Record<EmptyKind, ArtTone> = { default: 'primary', ok: 'ok', error: 'danger' };

export function EmptyState({
  title,
  text,
  actions,
  hint,
  kind = 'default',
  glyph = 'box',
  compact = false,
  headingLevel = 2,
  className,
}: EmptyStateProps) {
  const Heading = `h${String(headingLevel)}` as 'h2' | 'h3' | 'h4';
  return (
    <div
      className={cn(
        'empty',
        kind !== 'default' && `empty--${kind}`,
        compact && 'empty--compact',
        className,
      )}
    >
      <EmptyArt glyph={glyph} tone={TONE[kind]} />
      <Heading className="empty__title">{title}</Heading>
      {text ? <p className="empty__text">{text}</p> : null}
      {actions ? <div className="empty__actions">{actions}</div> : null}
      {hint ? <p className="empty__hint">{hint}</p> : null}
    </div>
  );
}
