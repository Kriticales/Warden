/**
 * Estado de atualização na linha da lista de Mods (SPEC T10): a versão nova e o botão
 * **Atualizar** (só com "Atualização disponível"; CA-T10-02), "Não foi possível verificar" com o
 * motivo, ou "Não verificado" por falta da chave da CurseForge. Em dia, fixado e arquivo local
 * não ganham marca na linha: o estado de cada um está nos detalhes.
 */
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../components/ui/button';
import type { InventoryItem, UpdateItem } from '../../lib/ipc/bindings';
import './updates.css';

export interface RowUpdateProps {
  item: InventoryItem;
  /** O resultado do relatório que vale para este item (ou `null`). */
  update: UpdateItem | null;
  onUpdate: (item: InventoryItem) => void;
}

export function RowUpdate({ item, update, onUpdate }: RowUpdateProps) {
  const { t } = useTranslation('atualizacoes');
  if (!update || item.pinned) return null;
  if (update.status === 'available' && update.newVersion) {
    return (
      <span className="modrow__update">
        <Button
          size="sm"
          icon={RefreshCw}
          aria-label={t('linha.atualizarPara', {
            name: item.name,
            version: update.newVersion.number,
          })}
          title={t('linha.paraVersao', { version: update.newVersion.number })}
          onClick={() => {
            onUpdate(item);
          }}
        >
          {t('linha.atualizar')}
        </Button>
        <span className="t-mono t-xs t-primary">{update.newVersion.number}</span>
      </span>
    );
  }
  if (update.status === 'failed' || update.status === 'notChecked') {
    const reason = update.reason ? t(`motivo.${update.reason}`) : undefined;
    return (
      <span className="tag tag--warn" title={reason}>
        {t(`estado.${update.status}`)}
      </span>
    );
  }
  return null;
}
