/**
 * Bloco "Atualização" do painel de detalhes (SPEC T07 e T10; protótipo `detailDrawer`): a versão
 * nova com o botão Atualizar, ou o estado do item (em dia, fixado, não verificado, não foi
 * possível verificar, não se aplica). O botão só fica habilitado com "Atualização disponível"
 * (CA-T10-02).
 */
import { RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import type { DetailBlockProps } from '../pack-editor/details/blocks';
import { useUpdatesReport } from './api';
import { effectiveStatus, reportItem } from './model';
import { ReviewDialog } from './ReviewDialog';

export function UpdateBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('atualizacoes');
  const report = useUpdatesReport(packId);
  const [reviewing, setReviewing] = useState(false);
  if (item.state !== 'ok') return null;
  const status = effectiveStatus(report.data, item);
  const update = reportItem(report.data, item);
  const available = status === 'available' && update?.newVersion;
  const reason = update?.reason ? t(`motivo.${update.reason}`) : null;
  const unavailableText: Record<string, string> = {
    upToDate: t('detalhes.emDia'),
    pinned: t('detalhes.fixado'),
    notApplicable: t('detalhes.naoAplica'),
    notChecked: reason ?? t('detalhes.naoVerificado'),
    failed: reason ?? '',
  };
  return (
    <>
      <Alert
        kind={available ? 'info' : status === 'failed' ? 'warn' : 'neutral'}
        compact
        title={
          available
            ? t('detalhes.disponivel', { version: update.newVersion?.number ?? '' })
            : t(`estado.${status}`)
        }
        actions={
          <Button
            size="sm"
            variant={available ? 'primary' : 'secondary'}
            icon={RefreshCw}
            disabled={!available}
            onClick={() => {
              setReviewing(true);
            }}
          >
            {available
              ? t('detalhes.atualizarPara', { version: update.newVersion?.number ?? '' })
              : t('detalhes.atualizar')}
          </Button>
        }
      >
        <div className="alert__text">
          {available
            ? t('detalhes.disponivelTexto', { current: update.current ?? item.version ?? '' })
            : (unavailableText[status] ?? '')}
        </div>
      </Alert>
      <ReviewDialog
        packId={packId}
        items={reviewing ? [item] : null}
        onClose={() => {
          setReviewing(false);
        }}
      />
    </>
  );
}
