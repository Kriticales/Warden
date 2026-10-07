/** "O que este formato perde" (SPEC T19): em linguagem simples, antes de gerar. */
import { useTranslation } from 'react-i18next';

import type { FormatAnalysis, FormatLoss } from '../../../lib/ipc/bindings';

export function useLossText(): (loss: FormatLoss) => string {
  const { t } = useTranslation('exportarFormatos');
  return (loss) => {
    switch (loss.kind) {
      case 'sideNotKept':
        return t('perdas.sideNotKept', { count: loss.count });
      case 'serverOnlyLeft':
        return t('perdas.serverOnlyLeft', { count: loss.count });
      case 'optionalTexts':
        return t('perdas.optionalTexts', { count: loss.count });
      case 'preserve':
        return t('perdas.preserve', { count: loss.count });
      case 'pinned':
        return t('perdas.pinned', { count: loss.count });
      case 'updateSources':
        return t('perdas.updateSources', { count: loss.count });
      case 'noAutoUpdate':
        return t('perdas.noAutoUpdate');
      case 'configsEverywhere':
        return t('perdas.configsEverywhere');
    }
  };
}

export function LossesPanel({ analysis }: { analysis: FormatAnalysis }) {
  const { t } = useTranslation('exportarFormatos');
  const text = useLossText();
  return (
    <section className="panel" aria-labelledby="exf-perdas">
      <h2 className="panel__title panel__title--sans" id="exf-perdas">
        {t('perdas.titulo')}
      </h2>
      <p className="t-sm t-2 mt-2">{t('perdas.intro')}</p>
      <ul className="mt-2 t-sm">
        {analysis.losses.map((loss) => (
          <li key={loss.kind}>{text(loss)}</li>
        ))}
      </ul>
    </section>
  );
}
