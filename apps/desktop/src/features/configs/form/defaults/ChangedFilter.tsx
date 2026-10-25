/**
 * O filtro "Mudou do padrão" (SPEC T12; esboço aprovado `design/bug-01/configs.html`; C-06), em
 * botão (sem caixa de seleção), no formulário do arquivo aberto e na coluna da seção.
 */
import { useTranslation } from 'react-i18next';

import { FilterChip } from '../../places/FilterChip';
import './defaults.css';

export interface ChangedFilterProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Quantas chaves (ou arquivos) mudaram do padrão. */
  count?: number | null;
  /** O rótulo longo ("Só o que mudou do padrão") em vez do curto. */
  long?: boolean;
}

export function ChangedFilter({ checked, onChange, count, long = false }: ChangedFilterProps) {
  const { t } = useTranslation('configsPadrao');
  const { t: tc } = useTranslation('configs');
  return (
    <FilterChip
      label={long ? t('filtro.rotulo') : tc('filtros.mudouCurto')}
      pressed={checked}
      count={count}
      title={t('filtro.dica')}
      onChange={onChange}
    />
  );
}
