/**
 * Confirmação de "Remover" (SPEC T06; protótipo `removeDialog`): o que sai do pack, os arquivos
 * apagados e quem depende dos itens ("Estes mods dependem dele: …"). O `items_remove_plan` traz
 * a cadeia inteira pelo grafo da D-07 e as dependências que ficam sem uso (P1-15), mostradas por
 * `RemovalDetails`; as marcadas saem junto.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import type { InventoryItem, PackId } from '../../../lib/ipc/bindings';
import { useRemovalPlan, useRemoveItems } from '../api';
import { RemovalDetails } from '../change-version/RemovalDetails';

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
  const { t: tv } = useTranslation('trocarVersao');
  const paths = items ? items.map((item) => item.path) : null;
  // Dependências sem uso marcadas para sair junto (some com o diálogo).
  const [orphans, setOrphans] = useState<string[]>([]);
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
  const extra = orphans.filter((path) => plan.data?.orphans.some((orphan) => orphan.path === path));
  const total = count + extra.length;
  return (
    <ConfirmDialog
      open={items !== null}
      onOpenChange={(open) => {
        if (!open) {
          setOrphans([]);
          onClose();
        }
      }}
      title={
        single
          ? t('mods.remover.tituloUm', { name: singleName })
          : t('mods.remover.tituloVarios', { count })
      }
      description={description}
      confirmLabel={
        extra.length > 0
          ? tv('remover.confirmarComOrfas', { count: total })
          : single
            ? t('mods.remover.confirmarUm', { name: singleName })
            : t('mods.remover.confirmarVarios', { count })
      }
      confirmingLabel={t('mods.remover.removendo')}
      cancelLabel={t('mods.remover.manter')}
      onConfirm={async () => {
        if (!paths) return;
        const all = [...paths, ...extra];
        await remove.mutateAsync(all);
        setOrphans([]);
        onRemoved(all);
      }}
    >
      {plan.isError ? <ErrorPanel compact error={plan.error} /> : null}
      {plan.data ? (
        <RemovalDetails plan={plan.data} orphans={extra} onOrphansChange={setOrphans} />
      ) : null}
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
