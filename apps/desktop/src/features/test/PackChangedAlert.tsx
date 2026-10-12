/**
 * Aviso passageiro do cabeçalho (SPEC T13 "Regras"): editar o pack com o jogo aberto é
 * permitido, e o cabeçalho avisa "O pack mudou desde o início do teste; as mudanças valem no
 * próximo teste." Clicar leva à tela do teste.
 */
import { Info } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../components/ui/icon';
import type { HeaderSlotProps } from '../pack-editor/header/slots';
import { useOpenTest } from './TestButton';
import { useTestStore } from './store';

export function PackChangedAlert({ packId }: HeaderSlotProps) {
  const { t } = useTranslation('teste');
  const changed = useTestStore((store) => store.packChanged[packId] === true);
  const openTest = useOpenTest();
  if (!changed) {
    return null;
  }
  return (
    <button
      type="button"
      className="headalert headalert--info"
      onClick={() => {
        openTest(packId);
      }}
    >
      <Icon icon={Info} size="sm" />
      {t('avisos.packMudou')}
    </button>
  );
}
