/**
 * "Excluir do pack" (SPEC T19): mostra a linha que vai para o `.packwizignore` antes de gravar.
 * O arquivo continua na pasta; só deixa de ir para quem joga.
 */
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { showToast } from '../../../components/ui/toast';
import type { PackId } from '../../../lib/ipc/bindings';
import { useExcludeFromPack } from '../api';
import { ignoreRule } from '../model';

export interface ExcludeTarget {
  path: string;
  folder: boolean;
}

export function ExcludeDialog({
  packId,
  target,
  onClose,
}: {
  packId: PackId;
  target: ExcludeTarget | null;
  onClose: () => void;
}) {
  const { t } = useTranslation('exportar');
  const exclude = useExcludeFromPack(packId);
  return (
    <ConfirmDialog
      open={target !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t('excluir.titulo')}
      description={target?.folder ? t('excluir.textoPasta') : t('excluir.texto')}
      confirmLabel={t('excluir.confirmar')}
      confirmingLabel={t('excluir.confirmando')}
      destructive={false}
      onConfirm={async () => {
        if (!target) return;
        await exclude.mutateAsync(target.path);
        showToast({ kind: 'ok', title: t('excluir.feito', { path: target.path }) });
      }}
    >
      {target ? (
        <>
          <code className="export-rule">{ignoreRule(target.path, target.folder)}</code>
          <p className="t-sm t-3">{t('excluir.desfazer')}</p>
        </>
      ) : null}
    </ConfirmDialog>
  );
}
