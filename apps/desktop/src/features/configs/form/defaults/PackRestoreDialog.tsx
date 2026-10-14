/**
 * A correção **Restaurar padrão** de um `W_CONFIG_RANGE` em Problemas (SPEC T14, CA-T14-10;
 * C-06): lê o arquivo do pack, mostra a diferença da chave restaurada e só grava com a
 * confirmação, pelo `config_write` com o `hash` da leitura (se o arquivo mudou por fora, nada é
 * gravado e o erro diz por quê). Depois de gravar, o relatório fica "velho" até a próxima
 * verificação.
 */
import { useNavigate } from '@tanstack/react-router';
import { FileText, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../../components/common/DiffView';
import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { LoadingState } from '../../../../components/common/LoadingState';
import { Button } from '../../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../../components/ui/dialog';
import { showToast } from '../../../../components/ui/toast';
import type { PackId } from '../../../../lib/ipc/bindings';
import { usePackRestorePreview, usePackRestoreWrite } from './api';

/** A chave a restaurar: o arquivo do pack e o caminho dela como o formulário mostra. */
export interface PackRestoreTarget {
  path: string;
  key: string;
}

export interface PackRestoreDialogProps {
  packId: PackId;
  target: PackRestoreTarget | null;
  onClose: () => void;
  /** Depois de gravar. */
  onRestored?: (() => void) | undefined;
}

export function PackRestoreDialog({ packId, target, onClose, onRestored }: PackRestoreDialogProps) {
  const { t } = useTranslation('configsPadrao');
  const navigate = useNavigate();
  const preview = usePackRestorePreview(
    packId,
    target?.path ?? '',
    target?.key ?? '',
    target !== null,
  );
  const write = usePackRestoreWrite(packId);
  const data = preview.data;

  const confirm = async () => {
    if (!target || !data) return;
    try {
      await write.restore(target.path, data);
      showToast({
        kind: 'ok',
        title: t('problema.feito', { chave: target.key, arquivo: target.path }),
      });
      onClose();
      onRestored?.();
    } catch {
      // O erro fica no diálogo (`write.error`).
    }
  };

  return (
    <Dialog
      open={target !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={t('problema.titulo', { chave: target?.key ?? '' })}
        description={t('problema.sub', { arquivo: target?.path ?? '' })}
        footerSplit
        footer={
          <>
            <Button
              variant="ghost"
              icon={FileText}
              onClick={() => {
                if (!target) return;
                onClose();
                void navigate({
                  to: '/packs/$packId/configs',
                  params: { packId },
                  search: { arquivo: target.path },
                });
              }}
            >
              {t('problema.abrir')}
            </Button>
            <span className="row">
              <DialogClose asChild>
                <Button variant="ghost">{t('problema.cancelar')}</Button>
              </DialogClose>
              <Button
                variant="primary"
                icon={RotateCcw}
                disabled={!data || data.after === data.before}
                loading={write.isPending}
                onClick={() => {
                  void confirm();
                }}
              >
                {write.isPending ? t('problema.gravando') : t('problema.confirmar')}
              </Button>
            </span>
          </>
        }
      >
        {preview.isError ? (
          <ErrorPanel compact title={t('problema.erro')} error={preview.error} />
        ) : !data ? (
          <LoadingState label={t('problema.carregando')} />
        ) : (
          <DiffView file={target?.path ?? ''} before={data.before} after={data.after} />
        )}
        {write.isError ? (
          <ErrorPanel compact title={t('problema.erro')} error={write.error} />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
