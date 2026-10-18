/**
 * "Memória do jogo" e "Coletas de memória" na faixa de desempenho (SPEC T13; L-11; protótipo
 * `teste-jogo`, `W.perfStrip`): a memória usada e a máxima com o mini-gráfico dos últimos
 * 5 minutos (colunas em aviso a partir de 90% do máximo), "na última coleta" no Java 25 com
 * Serial ou Parallel, e as coletas com o tempo parado ("Pausas de coleta" no ZGC e no
 * Shenandoah). Sem leitura, "sem leitura" e o motivo, sem número inventado.
 */
import { useTranslation } from 'react-i18next';

import type { HeapUnavailable } from '../../../../lib/ipc/bindings';
import { memoryParts } from '../format';
import { formatMemory } from '../../text';
import { BlockChart } from './BlockChart';
import { countText, gcTimeText, seriesLabel } from './format';
import type { HeapView } from './model';

/** A partir daqui as colunas do gráfico ficam em aviso (o mesmo limite do `W_LOW_HEAP`). */
const WARN_SHARE = 0.9;

/** O motivo de não haver leitura, numa frase. */
export function useReasonText(): (reason: HeapUnavailable) => string {
  const { t } = useTranslation('memoria');
  return (reason) =>
    reason.kind === 'argument'
      ? t('motivo.argumento', { argumento: reason.argument })
      : t('motivo.naoPublica');
}

export function MemoryItem({ view }: { view: HeapView }) {
  const { t } = useTranslation('memoria');
  const reasonText = useReasonText();
  if (view.kind !== 'reading') {
    return (
      <div className="perf__item perf__item--mem">
        <div className="perf__lbl">{t('faixa.memoria')}</div>
        <div className="perf__val perf__val--na">
          {view.kind === 'measuring' ? t('faixa.medindo') : t('faixa.semLeitura')}
        </div>
        {view.kind === 'unavailable' ? (
          <div className="perf__sub">{reasonText(view.reason)}</div>
        ) : null}
      </div>
    );
  }
  const { heap } = view;
  const { value, unit } = memoryParts(heap.usedMb);
  return (
    <div className="perf__item perf__item--mem">
      <div className="perf__lbl">{t('faixa.memoria')}</div>
      <div className="perf__val">
        {value}{' '}
        <small>{t('faixa.deMaximo', { unidade: unit, maximo: formatMemory(heap.maxMb) })}</small>
      </div>
      {heap.details.atLastGc ? <div className="perf__sub">{t('faixa.naUltimaColeta')}</div> : null}
      {heap.details.seriesMb.length > 0 ? (
        <BlockChart
          values={heap.details.seriesMb}
          max={heap.maxMb}
          warnAt={heap.maxMb * WARN_SHARE}
          label={seriesLabel(heap.details.seriesMb)}
        />
      ) : null}
    </div>
  );
}

export function GcItem({ view }: { view: HeapView }) {
  const { t } = useTranslation('memoria');
  const { t: tPerf } = useTranslation('desempenho');
  const pauses = view.kind === 'reading' && view.heap.details.countsPauses;
  return (
    <div className="perf__item">
      <div className="perf__lbl">{pauses ? t('faixa.pausas') : t('faixa.coletas')}</div>
      {view.kind === 'reading' ? (
        <>
          <div className="perf__val">{countText(view.heap.gcCount)}</div>
          <div className="perf__sub">
            {pauses
              ? t('faixa.paradoEmPausas', { tempo: gcTimeText(view.heap.gcMs) })
              : t('faixa.paradoEmColetas', { tempo: gcTimeText(view.heap.gcMs) })}
          </div>
        </>
      ) : (
        <>
          <div className="perf__val perf__val--na">
            {view.kind === 'measuring' ? t('faixa.medindo') : tPerf('vazio')}
          </div>
          {view.kind === 'unavailable' ? (
            <div className="perf__sub">{t('faixa.semLeitura')}</div>
          ) : null}
        </>
      )}
    </div>
  );
}
