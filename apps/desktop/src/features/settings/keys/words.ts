/**
 * As palavras de cada chave nas frases ("a chave da CurseForge", "o token do GitHub") e a
 * hora do último teste ("hoje às 21:40").
 */
import { useTranslation } from 'react-i18next';

import type { SecretKind } from '../../../lib/ipc/bindings';
import { isToken, whenParts } from './model';

export function useKeyWords(kind: SecretKind) {
  const { t } = useTranslation('configuracoes');
  const token = isToken(kind);
  return {
    token,
    name: t(`chaves.nomes.${kind}`),
    de: t(`chaves.de.${kind}`),
  };
}

/** "hoje às 21:40", "ontem às 9:05", "em 03/10/2026 às 14:00" (ou `null`). */
export function useWhen(): (iso: string | undefined) => string | null {
  const { t } = useTranslation('configuracoes');
  return (iso) => {
    if (!iso) return null;
    const parts = whenParts(iso, Date.now());
    if (!parts) return null;
    if (parts.relation === 'today') return t('chaves.quandoHoje', { hora: parts.hora });
    if (parts.relation === 'yesterday') return t('chaves.quandoOntem', { hora: parts.hora });
    return t('chaves.quandoData', { data: parts.data, hora: parts.hora });
  };
}
