/** Na lista de Conversas: quantas propostas da IA esperam a decisão (protótipo `ia`). */
import { useTranslation } from 'react-i18next';

export function PendingCount({ count }: { count: number }) {
  const { t } = useTranslation('iaPropostas');
  if (count === 0) {
    return null;
  }
  return <div className="t-xs t-3">{t('lista.pendentes', { count })}</div>;
}
