/** "O que vai no arquivo": quantos mods por referência e quantos arquivos de terceiros dentro. */
import { useTranslation } from 'react-i18next';

import type { FormatAnalysis } from '../../../lib/ipc/bindings';
import { embeddedCount, isSwapped, type FormatDecisions } from './model';

export function SummaryPanel({
  analysis,
  decisions,
}: {
  analysis: FormatAnalysis;
  decisions: FormatDecisions;
}) {
  const { t } = useTranslation('exportarFormatos');
  const embedded = embeddedCount(analysis, decisions);
  const swapped = analysis.items.filter((item) => isSwapped(item, decisions)).length;
  const references = analysis.references + swapped;
  return (
    <section className="panel" aria-labelledby="exf-resumo">
      <h2 className="panel__title panel__title--sans" id="exf-resumo">
        {t('resumo.titulo')}
      </h2>
      <ul className="mt-3 t-sm">
        <li>
          {references > 0
            ? t('resumo.referencias', { count: references })
            : t('resumo.nenhumaReferencia')}
        </li>
        <li>
          {embedded > 0 ? t('resumo.embutidos', { count: embedded }) : t('resumo.nenhumEmbutido')}
        </li>
        <li>{t('resumo.configs')}</li>
      </ul>
    </section>
  );
}
