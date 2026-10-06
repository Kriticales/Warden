/** Contador da seção Mods no menu lateral: itens no pack (SPEC T05). */
import { useTranslation } from 'react-i18next';

import type { PackId } from '../../../lib/ipc/bindings';
import { useInventory } from '../api';
import type { SectionCount } from '../sections';
import { itemsInPack } from './model';

export function useModsCount(packId: PackId): SectionCount | null {
  const { t } = useTranslation('editor');
  const inventory = useInventory(packId);
  if (!inventory.isSuccess) {
    return null;
  }
  const value = itemsInPack(inventory.data.items);
  return { value, kind: 'neutral', label: t('secoes.contagemItens', { count: value }) };
}
