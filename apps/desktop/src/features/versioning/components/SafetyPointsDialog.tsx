/**
 * Pontos de segurança (SPEC T17, P1): as cópias automáticas do pack feitas antes de ações que
 * apagam ou substituem conteúdo, com data e motivo ("antes de voltar para 1.2.0") e Recuperar.
 */
import { RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type { PackId, SafetyPoint } from '../../../lib/ipc/bindings';
import { useRecoverSafetyPoint, useSafetyPoints } from '../api';
import { whenParts } from '../model';

/** "hoje, 15:02", "ontem, 15:02" ou "28/09/2026, 15:02" como texto simples. */
function useWhenText() {
  const { t } = useTranslation('versoes');
  return (iso: string): string => {
    const when = whenParts(iso);
    if (when === null) {
      return iso;
    }
    return when.relation === 'today'
      ? t('data.hoje', { hora: when.time })
      : when.relation === 'yesterday'
        ? t('data.ontem', { hora: when.time })
        : t('data.outro', { data: when.date, hora: when.time });
  };
}

export function SafetyPointsDialog({
  packId,
  canWrite,
  onClose,
}: {
  packId: PackId;
  canWrite: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('versoes');
  const when = useWhenText();
  const points = useSafetyPoints(packId, true);
  const recover = useRecoverSafetyPoint(packId);
  const [chosen, setChosen] = useState<SafetyPoint | null>(null);

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <DialogContent
        size="lg"
        title={t('pontos.titulo')}
        description={t('pontos.descricao')}
        footer={
          <Button variant="ghost" onClick={onClose}>
            {t('pontos.fechar')}
          </Button>
        }
      >
        {points.isPending ? (
          <LoadingState inline label={t('pontos.carregando')} />
        ) : points.isError ? (
          <ErrorPanel
            compact
            error={points.error}
            onRetry={() => {
              void points.refetch();
            }}
          />
        ) : points.data.length === 0 ? (
          <p className="t-sm t-3">{t('pontos.vazio')}</p>
        ) : (
          <ul className="safety">
            {points.data.map((point) => (
              <li key={point.name} className="safety__row">
                <div>
                  <div className="safety__reason">{point.reason}</div>
                  <div className="t-sm t-3">{when(point.createdAt)}</div>
                </div>
                <Button
                  size="sm"
                  icon={RotateCcw}
                  disabled={!canWrite}
                  aria-label={t('pontos.recuperarRotulo', { data: when(point.createdAt) })}
                  onClick={() => {
                    setChosen(point);
                  }}
                >
                  {t('pontos.recuperar')}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
      <ConfirmDialog
        open={chosen !== null}
        onOpenChange={(open) => {
          if (!open) {
            setChosen(null);
          }
        }}
        title={t('pontos.confirmarTitulo', { data: chosen ? when(chosen.createdAt) : '' })}
        description={t('pontos.confirmarTexto', { motivo: chosen?.reason ?? '' })}
        confirmLabel={t('pontos.confirmar')}
        confirmingLabel={t('pontos.recuperando')}
        destructive={false}
        onConfirm={async () => {
          if (chosen === null) {
            return;
          }
          await recover.mutateAsync(chosen.name);
          showToast({
            kind: 'ok',
            title: t('pontos.feito'),
            text: t('pontos.feitoTexto'),
          });
          void points.refetch();
        }}
      />
    </Dialog>
  );
}
