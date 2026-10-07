/**
 * "Verificar atualizações" (SPEC T10; protótipo `pageHead` da seção Mods): consulta o Modrinth
 * em lote e a CurseForge por projeto. A dica diz quando foi a última verificação.
 */
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../components/ui/button';
import { Tooltip } from '../../components/ui/tooltip';
import { formatTime } from '../../lib/format';
import type { PackId } from '../../lib/ipc/bindings';
import { useCheckUpdates, useUpdatesReport } from './api';

export function CheckButton({ packId }: { packId: PackId }) {
  const { t } = useTranslation('atualizacoes');
  const report = useUpdatesReport(packId);
  const check = useCheckUpdates(packId);
  const checkedAt = report.data ? Date.parse(report.data.checkedAt) : Number.NaN;
  const hint = Number.isFinite(checkedAt)
    ? t('ultimaVerificacao', { quando: formatTime(checkedAt) })
    : t('nuncaVerificado');
  return (
    <Tooltip content={check.isError ? t('verificacaoFalhou') : hint}>
      <Button
        icon={RefreshCw}
        loading={check.isPending}
        onClick={() => {
          check.mutate();
        }}
      >
        {check.isPending ? t('verificando') : t('verificar')}
      </Button>
    </Tooltip>
  );
}
