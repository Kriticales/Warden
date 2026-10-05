/**
 * Cabeçalho de página (DESIGN-SYSTEM §5): título curto em pixel, uma linha dizendo o que há
 * ali e as ações à direita. O título recebe o foco quando a tela muda (`data-page-title`).
 */
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { PAGE_TITLE_ATTRIBUTE } from './focus';

export interface PageHeadProps {
  title: ReactNode;
  sub?: ReactNode;
  actions?: ReactNode;
  /** Título em Manrope em vez de pixel (frases longas). */
  sans?: boolean;
  /** "← Voltar para …" acima do título, nas subpáginas. */
  back?: ReactNode;
}

export function PageHead({ title, sub, actions, sans = false, back }: PageHeadProps) {
  return (
    <div className="pagehead">
      <div>
        {back ? <div className="back-link">{back}</div> : null}
        <PageTitle className={cn('pagehead__title', sans && 'pagehead__title--sans')}>
          {title}
        </PageTitle>
        {sub ? <p className="pagehead__sub">{sub}</p> : null}
      </div>
      {actions ? <div className="pagehead__actions">{actions}</div> : null}
    </div>
  );
}

/** O `<h1>` da tela, focável por script (sem anel de foco). */
export function PageTitle({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <h1 className={className} tabIndex={-1} {...{ [PAGE_TITLE_ATTRIBUTE]: '' }}>
      {children}
    </h1>
  );
}
