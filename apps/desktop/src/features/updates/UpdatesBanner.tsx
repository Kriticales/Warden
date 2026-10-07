/**
 * Faixa acima da lista de Mods (protótipo `mods-banner`): "N atualizações disponíveis" com
 * Revisar e atualizar, e os avisos de itens que não foram verificados. Falha de rede nunca é
 * "Em dia" (CA-T10-04): o aviso diz quantos itens ficaram sem resposta.
 */
import { Link } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import type { InventoryItem, PackId, ModUpdateReport } from '../../lib/ipc/bindings';
import { useCheckUpdates } from './api';
import { countUpdates } from './model';

export interface UpdatesBannerProps {
  packId: PackId;
  report: ModUpdateReport | null | undefined;
  items: readonly InventoryItem[];
  onReview: () => void;
}

export function UpdatesBanner({ packId, report, items, onReview }: UpdatesBannerProps) {
  const { t } = useTranslation('atualizacoes');
  const check = useCheckUpdates(packId);
  if (!report) return null;
  const counts = countUpdates(report, items);
  return (
    <>
      {counts.available > 0 ? (
        <Alert
          kind="neutral"
          compact
          className="mods-banner"
          title={t('faixa.titulo', { count: counts.available })}
          actions={
            <Button size="sm" onClick={onReview}>
              {t('faixa.revisar')}
            </Button>
          }
        >
          <div className="alert__text">{t('faixa.texto')}</div>
        </Alert>
      ) : null}
      {counts.failed > 0 ? (
        <Alert
          kind="warn"
          compact
          className="mods-banner"
          title={t('faixa.semVerificar', { count: counts.failed })}
          actions={
            <Button
              size="sm"
              icon={RefreshCw}
              loading={check.isPending}
              onClick={() => {
                check.mutate();
              }}
            >
              {t('faixa.tentarDeNovo')}
            </Button>
          }
        >
          <div className="alert__text">{t('faixa.semVerificarTexto')}</div>
        </Alert>
      ) : null}
      {counts.keyProblem > 0 ? (
        <Alert
          kind="info"
          compact
          className="mods-banner"
          title={t('faixa.semChave', { count: counts.keyProblem })}
          actions={
            <Button size="sm" asChild>
              <Link to="/configuracoes">{t('motivo.abrirConfiguracoes')}</Link>
            </Button>
          }
        >
          <div className="alert__text">{t('faixa.semChaveTexto')}</div>
        </Alert>
      ) : null}
    </>
  );
}
