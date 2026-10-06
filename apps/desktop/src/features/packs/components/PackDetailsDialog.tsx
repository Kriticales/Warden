/**
 * "Ver detalhes" de um pack ilegível (T02, estado "Pack inválido"): a pasta e o erro técnico
 * da leitura do `pack.toml`, com Copiar (o painel de erro comum) e Mostrar na pasta.
 */
import { FolderOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type { AppError, PackRow } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useRevealPack } from '../api';

export function PackDetailsDialog({
  row,
  onOpenChange,
}: {
  row: PackRow;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation('packs');
  const { t: tc } = useTranslation();
  const reveal = useRevealPack();
  const error: AppError = {
    code: { domain: 'project', code: 'INVALID_PACK' },
    params: {},
    detail: row.detail,
    retryable: false,
    operationId: null,
  };
  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent
        title={t('detalhes.titulo', { name: row.name || row.path })}
        description={t('detalhes.texto')}
        footer={
          <>
            <Button
              icon={FolderOpen}
              onClick={() => {
                reveal.mutate(row.id, {
                  onError: (cause) => {
                    showToast({ kind: 'danger', title: appErrorMessage(toAppError(cause)) });
                  },
                });
              }}
            >
              {t('linha.mostrarNaPasta')}
            </Button>
            <DialogClose asChild>
              <Button variant="primary">{tc('acoes.fechar')}</Button>
            </DialogClose>
          </>
        }
      >
        <dl className="kv">
          <dt>{t('detalhes.pasta')}</dt>
          <dd className="path">{row.path}</dd>
        </dl>
        <ErrorPanel compact error={error} className="mt-3" />
      </DialogContent>
    </Dialog>
  );
}
