/**
 * A caixa "Só o que mudou do padrão" (SPEC T12; protótipos `configs` e `configs-busca`; C-06),
 * usada no formulário do arquivo aberto e na busca em todas as configs.
 */
import { useTranslation } from 'react-i18next';

import { Checkbox } from '../../../settings/components/fields';
import './defaults.css';

export interface ChangedFilterProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Linha de explicação abaixo (a contagem, no formulário). */
  desc?: string | undefined;
}

export function ChangedFilter({ checked, onChange, desc }: ChangedFilterProps) {
  const { t } = useTranslation('configsPadrao');
  return (
    <div className="cfg-changed-filter" title={t('filtro.dica')}>
      <Checkbox
        label={t('filtro.rotulo')}
        {...(desc === undefined ? {} : { desc })}
        checked={checked}
        onChange={onChange}
      />
    </div>
  );
}
