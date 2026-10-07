/**
 * "Ver diferenças para o estado atual" (SPEC T17): o que mudou no pack da versão escolhida até
 * agora, incluindo o que ainda não foi salvo.
 */
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { PackId } from '../../../lib/ipc/bindings';
import { useVersionChanges } from '../api';
import { ChangeList } from './ChangeList';

export function DiffDialog({
  packId,
  version,
  onClose,
}: {
  packId: PackId;
  version: string;
  onClose: () => void;
}) {
  const { t } = useTranslation('versoes');
  const changes = useVersionChanges(packId, version, true);
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
        title={t('diferencas.titulo', { version })}
        description={t('diferencas.descricao', { version })}
        footer={
          <Button variant="ghost" onClick={onClose}>
            {t('diferencas.fechar')}
          </Button>
        }
      >
        {changes.isPending ? (
          <LoadingState inline label={t('diferencas.carregando')} />
        ) : changes.isError ? (
          <ErrorPanel
            compact
            error={changes.error}
            onRetry={() => {
              void changes.refetch();
            }}
          />
        ) : changes.data.files.length === 0 ? (
          <p>{t('diferencas.igual', { version })}</p>
        ) : (
          <ChangeList changes={changes.data} />
        )}
      </DialogContent>
    </Dialog>
  );
}
