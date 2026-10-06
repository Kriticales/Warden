/**
 * Detalhes do item num painel lateral, sem sair da lista (SPEC T07; protótipo `detailDrawer`).
 * O conteúdo são os blocos de `blocks.ts`, na ordem; o que vem do `.pw.toml` aparece na hora e
 * o que vem da fonte (Modrinth do cache, CurseForge ao vivo) chega depois.
 */
import { Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Sheet, SheetContent } from '../../../components/ui/sheet';
import type { InventoryItem, PackId } from '../../../lib/ipc/bindings';
import { useItemDetails } from '../api';
import { orderedBlocks } from './blocks';

export interface ItemDrawerProps {
  packId: PackId;
  /** O item aberto; `null` = painel fechado. */
  item: InventoryItem | null;
  onClose: () => void;
  onRemove: (item: InventoryItem) => void;
}

export function ItemDrawer({ packId, item, onClose, onRemove }: ItemDrawerProps) {
  return (
    <Sheet
      open={item !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {item ? <DrawerBody packId={packId} item={item} onRemove={onRemove} /> : null}
    </Sheet>
  );
}

function DrawerBody({
  packId,
  item,
  onRemove,
}: {
  packId: PackId;
  item: InventoryItem;
  onRemove: (item: InventoryItem) => void;
}) {
  const { t } = useTranslation('editor');
  // Arquivo inválido ou fora do índice: tudo o que há está na lista.
  const fetchable = item.state === 'ok';
  const details = useItemDetails(packId, fetchable ? item.path : null);
  return (
    <SheetContent
      title={t(`detalhes.tituloTipo.${item.kind}`)}
      footer={
        <Button
          variant="danger-ghost"
          icon={Trash2}
          onClick={() => {
            onRemove(item);
          }}
        >
          {t('detalhes.remover')}
        </Button>
      }
    >
      {fetchable && details.isPending ? (
        <LoadingState inline label={t('detalhes.carregando')} />
      ) : null}
      {details.isError ? (
        <ErrorPanel
          compact
          error={details.error}
          onRetry={() => {
            void details.refetch();
          }}
        />
      ) : null}
      {orderedBlocks().map(({ id, component: Block }) => (
        <Block key={id} packId={packId} item={item} details={details.data} />
      ))}
    </SheetContent>
  );
}
