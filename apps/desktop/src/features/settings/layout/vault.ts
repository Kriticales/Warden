/**
 * O nome do cofre onde as chaves ficam (ADR-0069): "Cofre do Windows" no Windows (o
 * Gerenciador de Credenciais) e "cofre do sistema" no Linux (Secret Service; só a CI roda lá).
 */
import { useTranslation } from 'react-i18next';

import { useAppInfo } from '../../about/hooks/useAppInfo';

export function useIsWindows(): boolean {
  const info = useAppInfo();
  // Enquanto a informação não chega, vale a plataforma principal.
  return (info.data?.platform ?? 'windows') === 'windows';
}

export function useVaultName(): string {
  const { t } = useTranslation('configuracoes');
  return useIsWindows() ? t('cofre.windows') : t('cofre.sistema');
}
