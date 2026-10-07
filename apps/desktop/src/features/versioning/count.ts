/**
 * Contador da seção Histórico no menu do pack (SPEC T05): as alterações não salvas, em cor de
 * atenção, e nada quando o pack está igual à última versão salva. Vem da linha do pack, a mesma
 * do botão "Salvar versão · N alterações" do cabeçalho.
 */
import { useTranslation } from 'react-i18next';

import type { PackId } from '../../lib/ipc/bindings';
import type { SectionCount } from '../pack-editor/sections';
import { usePack } from '../packs/api';

export function useUnsavedCount(packId: PackId): SectionCount | null {
  const { t } = useTranslation('versoes');
  const pack = usePack(packId);
  const value = pack.data?.unsavedFiles ?? 0;
  if (value === 0) {
    return null;
  }
  return { value, kind: 'warn', label: t('historico.contagem', { count: value }) };
}
