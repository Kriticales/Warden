/**
 * A frase de cada alerta da prévia (`ExportAlert`): uma por `kind`; a higiene usa a frase do
 * motivo de Abrir pack (`packs.motivo`), a mesma da tabela de limpeza.
 */
import { useTranslation } from 'react-i18next';

import type { ExportAlert } from '../../../lib/ipc/bindings';
import { causeText } from '../../packs/lib/pack-list';

export function useAlertText(): (alert: ExportAlert) => string {
  const { t } = useTranslation(['exportar', 'packs']);
  return (alert) => {
    switch (alert.kind) {
      case 'largeFile':
        return t('alertas.largeFile');
      case 'optionsOverridesPreferences':
        return t('alertas.optionsOverridesPreferences');
      case 'looseRootFile':
        return t('alertas.looseRootFile');
      case 'hygiene': {
        const text = causeText(alert.cause);
        const motivo =
          'count' in text
            ? t(`motivo.${text.key}`, { ns: 'packs', count: text.count })
            : t(`motivo.${text.key}`, { ns: 'packs' });
        return t('alertas.higiene', { motivo });
      }
    }
  };
}
