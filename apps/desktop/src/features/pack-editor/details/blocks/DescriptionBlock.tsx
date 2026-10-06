/**
 * Descrição do projeto (SPEC T07): Modrinth em Markdown, CurseForge em HTML, as duas
 * higienizadas (CA-T07-01: `<script>` e `onerror=` nunca rodam; links abrem no navegador).
 */
import { useTranslation } from 'react-i18next';

import { SafeHtml } from '../../../../components/common/SafeHtml';
import { SafeMarkdown } from '../../../../components/common/SafeMarkdown';
import type { DetailBlockProps } from '../blocks';

export function DescriptionBlock({ item, details }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  if (!details || details.source === 'none' || item.state === 'invalid') {
    return null;
  }
  const summary = details.summary ?? item.summary;
  const body = details.description;
  if (!body && !summary) {
    return details.source === 'offline' ||
      details.source === 'noKey' ||
      details.source === 'unavailable' ? null : (
      <p className="t-sm t-3">{t('detalhes.semDescricao')}</p>
    );
  }
  return (
    <div className="itemdesc">
      {summary ? <p className="t-sm t-2">{summary}</p> : null}
      {body ? (
        // A descrição do projeto costuma ser longa: fica recolhida, para versão, arquivo e lado
        // aparecerem logo (protótipo `detailDrawer`).
        <details className="disclosure">
          <summary>{t('detalhes.descricaoCompleta')}</summary>
          {details.descriptionFormat === 'html' ? (
            <SafeHtml html={body} className="prose itemdesc__body" />
          ) : (
            <SafeMarkdown className="prose itemdesc__body">{body}</SafeMarkdown>
          )}
        </details>
      ) : null}
    </div>
  );
}
