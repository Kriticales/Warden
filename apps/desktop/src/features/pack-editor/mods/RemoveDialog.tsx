/**
 * Confirmação de "Remover" (SPEC T06; protótipo `removeDialog`): o que sai do pack, os arquivos
 * apagados e quem depende dos itens ("Estes mods dependem dele: …"). Os dependentes vêm de
 * `items_remove_plan` (diretos; a P1-15 troca pela consulta transitiva da D-07).
 */
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { showToast } from '../../../components/ui/toast';
import type { InventoryItem, PackId } from '../../../lib/ipc/bindings';
import { useRemovalPlan, useRemoveItems } from '../api';

export interface RemoveDialogProps {
  packId: PackId;
  /** Os itens a remover; `null` = fechado. */
  items: readonly InventoryItem[] | null;
  onClose: () => void;
  /** Depois de remover (limpa a seleção, fecha os detalhes). */
  onRemoved: (paths: string[]) => void;
}

export function RemoveDialog({ packId, items, onClose, onRemoved }: RemoveDialogProps) {
  const { t } = useTranslation('editor');
  const paths = items ? items.map((item) => item.path) : null;
  const plan = useRemovalPlan(packId, paths);
  const remove = useRemoveItems(packId);
  const count = items?.length ?? 0;
  const single = count === 1 ? items?.[0] : undefined;
  const singleName = single?.name ?? '';
  const dependents = plan.data?.dependents ?? [];
  const names = dependents.map((dependent) => dependent.name).join(', ');

  let description: string;
  if (plan.isPending) {
    description = t('mods.remover.carregando');
  } else if (dependents.length === 0) {
    description = t('mods.remover.semDependentes');
  } else if (single) {
    description = t('mods.remover.dependentesUm', { count: dependents.length, nomes: names });
  } else {
    description = t('mods.remover.dependentesVarios', { count: dependents.length, nomes: names });
  }

  const files = plan.data?.targets.flatMap((target) => target.files) ?? [];
  return (
    <ConfirmDialog
      open={items !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={
        single
          ? t('mods.remover.tituloUm', { name: singleName })
          : t('mods.remover.tituloVarios', { count })
      }
      description={description}
      confirmLabel={
        single
          ? t('mods.remover.confirmarUm', { name: singleName })
          : t('mods.remover.confirmarVarios', { count })
      }
      confirmingLabel={t('mods.remover.removendo')}
      cancelLabel={t('mods.remover.manter')}
      onConfirm={async () => {
        if (!paths) return;
        await remove.mutateAsync(paths);
        onRemoved(paths);
        showToast({ kind: 'ok', title: t('mods.remover.removidos', { count }) });
      }}
    >
      {plan.isError ? <ErrorPanel compact error={plan.error} /> : null}
      {files.length > 0 ? (
        <details className="disclosure">
          <summary>{t('mods.remover.arquivos')}</summary>
          <ul className="removelist">
            {files.map((file) => (
              <li key={file} className="path">
                {file}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      <p className="t-sm t-3">{t('mods.remover.volta')}</p>
    </ConfirmDialog>
  );
}
