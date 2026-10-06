/** Novidades da versão instalada (SPEC T07), recolhidas; higienizadas como a descrição. */
import { useTranslation } from 'react-i18next';

import { SafeMarkdown } from '../../../../components/common/SafeMarkdown';
import type { DetailBlockProps } from '../blocks';

export function ChangelogBlock({ item, details }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  const changelog = details?.changelog?.trim();
  if (!changelog) {
    return null;
  }
  return (
    <details className="disclosure">
      <summary>
        {t('detalhes.novidades', { version: details?.version?.number ?? item.version ?? '' })}
      </summary>
      <SafeMarkdown className="prose itemdesc__body">{changelog}</SafeMarkdown>
    </details>
  );
}
