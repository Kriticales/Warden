/**
 * Filtro em botão (esboço aprovado `design/bug-01/configs.html`: "os filtros viram botões do
 * mesmo tamanho", sem caixa de seleção): `aria-pressed` diz se está ligado e a contagem vai ao
 * lado. `place="test"` é o filtro do teste local (o quadrado âmbar, cor do teste).
 */
import { cn } from '../../../lib/cn';

export interface FilterChipProps {
  label: string;
  pressed: boolean;
  count?: number | null | undefined;
  title?: string | undefined;
  place?: 'test' | undefined;
  onChange: (pressed: boolean) => void;
}

export function FilterChip({ label, pressed, count, title, place, onChange }: FilterChipProps) {
  return (
    <button
      type="button"
      className={cn('fchip', place === 'test' && 'fchip--test')}
      aria-pressed={pressed}
      title={title}
      onClick={() => {
        onChange(!pressed);
      }}
    >
      {place === 'test' ? <span className="fchip__sq" aria-hidden="true" /> : null}
      {label}
      {count === undefined || count === null ? null : <span className="fchip__n">{count}</span>}
    </button>
  );
}
