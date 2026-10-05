import { cn } from '../../lib/cn';

/** Carregador em blocos: 4 quadrados que acendem em sequência (decorativo; HANDOFF §6). */
export function Loader({ className }: { className?: string }) {
  return (
    <span className={cn('loader', className)} aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
    </span>
  );
}
