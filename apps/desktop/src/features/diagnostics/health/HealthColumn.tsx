/**
 * Coluna Saúde de Meus packs (SPEC T02, CA-T02-05; decisão D24; protótipo `packs`,
 * `packTableHead({ withHealth })`): a última nota guardada de cada pack, com a mesma faixa da
 * seção Problemas. Abrir a lista só lê as notas guardadas (`health_list`, uma vez para a tabela
 * inteira, em `PacksPage`), nunca roda o diagnóstico; pack sem nota mostra "Sem dados".
 */
import { useTranslation } from 'react-i18next';

import { Skeleton } from '../../../components/common/LoadingState';
import { Tooltip } from '../../../components/ui/tooltip';
import { HealthMeter, type HealthValue } from './HealthMeter';
import './health.css';

/** O título da coluna, com a dica do que a nota é. */
export function HealthColumnHeader() {
  const { t } = useTranslation('saude');
  return (
    <th scope="col">
      <Tooltip content={t('colunaDica')}>
        <button type="button" className="health-cell__tip">
          {t('coluna')}
        </button>
      </Tooltip>
    </th>
  );
}

/** A célula de um pack (`value`: `null` sem dados, `undefined` enquanto as notas são lidas). */
export function HealthCell({ value }: { value: HealthValue | null | undefined }) {
  if (value === undefined) {
    return (
      <td aria-busy="true">
        <Skeleton width="60%" />
      </td>
    );
  }
  return (
    <td>
      <HealthMeter value={value} size="sm" />
    </td>
  );
}
