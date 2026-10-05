/**
 * Carregando (HANDOFF §3): o carregador em blocos com uma frase dizendo o que está sendo lido,
 * num `role="status"` para o leitor de tela. Quando a forma do conteúdo é conhecida, prefira
 * `Skeleton` (mesma forma do resultado); spinner no meio da tela não existe no Warden.
 */
import type { CSSProperties, ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { Loader } from '../ui/loader';

export interface LoadingStateProps {
  /** O que está sendo carregado ("Lendo as tarefas…"). */
  label: ReactNode;
  /** Em bloco (ocupa a linha, com respiro) ou em linha. Padrão: bloco. */
  inline?: boolean;
  className?: string | undefined;
}

export function LoadingState({ label, inline = false, className }: LoadingStateProps) {
  return (
    <div
      className={cn('loading', !inline && 'loading--block', className)}
      role="status"
      aria-live="polite"
    >
      <Loader />
      <span>{label}</span>
    </div>
  );
}

export interface SkeletonProps {
  /** Forma: linha de texto, ícone (32 px) ou botão pequeno. */
  shape?: 'line' | 'tile' | 'btn';
  /** Largura (`--w`), por exemplo "60%". */
  width?: string;
  className?: string | undefined;
}

/** Bloco de espera com a forma do conteúdo final. Decorativo: quem anuncia é o `aria-busy`. */
export function Skeleton({ shape = 'line', width, className }: SkeletonProps) {
  return (
    <span
      aria-hidden="true"
      className={cn('skeleton', `skeleton--${shape}`, className)}
      style={width ? ({ '--w': width } as CSSProperties) : undefined}
    />
  );
}
