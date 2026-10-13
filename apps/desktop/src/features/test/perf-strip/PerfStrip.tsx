/**
 * Faixa de desempenho do teste (SPEC T13; ARCHITECTURE §7.7; L-10; protótipo `teste-jogo`,
 * `W.perfStrip`): acima do console com o jogo aberto, a RAM do processo (com o pico), "Abriu
 * em" (com a diferença para a última versão testada) e "Entrou no mundo em", atualizados a cada
 * amostra. No resultado do teste, o mesmo resumo com os valores gravados na sessão.
 *
 * A L-11 acrescenta, à esquerda, a "Memória do jogo" com o gráfico e as "Coletas de memória"
 * (`perf-strip/heap/`).
 */
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { ResultBlockProps } from '../result-blocks';
import type { TestRun } from '../store';
import { comparisonText, loadTimeText, memoryParts, memoryText } from './format';
import { fromSession, type PerfState } from './model';
import './perf-strip.css';

/**
 * Um valor da faixa: rótulo, número (com a unidade menor) e uma linha de apoio. Sem número
 * (`missing`), o texto no lugar dele sai sem destaque.
 */
function Item({
  label,
  value,
  sub,
  missing = false,
}: {
  label: string;
  value: ReactNode;
  sub?: string | null;
  missing?: boolean;
}) {
  return (
    <div className="perf__item">
      <div className="perf__lbl">{label}</div>
      <div className={missing ? 'perf__val perf__val--na' : 'perf__val'}>{value}</div>
      {sub ? <div className="perf__sub">{sub}</div> : null}
    </div>
  );
}

/** Memória com a unidade menor: "4,2 <small>GB</small>". */
function Memory({ mb }: { mb: number }) {
  const { value, unit } = memoryParts(mb);
  return (
    <>
      {value} <small>{unit}</small>
    </>
  );
}

/** A diferença de "Abriu em" para a última sessão de outra versão, quando há. */
function comparison(perf: PerfState): string | null {
  return perf.loadedMs !== null && perf.previous
    ? comparisonText(perf.loadedMs, perf.previous)
    : null;
}

/** A faixa com o jogo aberto. */
export function PerfStrip({ perf }: { perf: PerfState | null }) {
  const { t } = useTranslation('desempenho');
  const rss = perf?.rssMb ?? null;
  const peak = perf?.peakRssMb ?? null;
  const loaded = perf?.loadedMs ?? null;
  const world = perf?.worldMs ?? null;
  return (
    <section className="perf perf--process" aria-label={t('faixa.rotulo')}>
      <Item
        label={t('faixa.ram')}
        value={rss === null ? t('faixa.medindo') : <Memory mb={rss} />}
        missing={rss === null}
        sub={peak === null ? null : t('faixa.ramSub', { pico: memoryText(peak) })}
      />
      <Item
        label={t('faixa.abriuEm')}
        value={loaded === null ? t('faixa.carregando') : loadTimeText(loaded)}
        missing={loaded === null}
        sub={perf ? comparison(perf) : null}
      />
      <Item
        label={t('faixa.mundo')}
        value={world === null ? t('vazio') : loadTimeText(world)}
        missing={world === null}
        sub={world === null ? t('faixa.aindaNao') : null}
      />
    </section>
  );
}

/** A faixa do teste em andamento (amostras do canal). */
export function LivePerfStrip({ run }: { run: TestRun }) {
  return <PerfStrip perf={run.perf} />;
}

/** "Abriu em …" no resultado do teste (registro `result-blocks.ts`). */
export function PerfSummary({ session }: ResultBlockProps) {
  const { t } = useTranslation('desempenho');
  if (!session.perf) {
    return null;
  }
  const perf = fromSession(session.perf);
  const notLoaded =
    session.outcome === 'crashed' ? t('resultado.travouAntes') : t('resultado.naoCarregou');
  return (
    <section className="perf perf--process" aria-label={t('resultado.rotulo')}>
      <Item
        label={t('resultado.ramPico')}
        value={perf.peakRssMb === null ? t('vazio') : <Memory mb={perf.peakRssMb} />}
        missing={perf.peakRssMb === null}
        sub={perf.peakRssMb === null ? t('resultado.semMedida') : t('resultado.ramSub')}
      />
      <Item
        label={t('faixa.abriuEm')}
        value={perf.loadedMs === null ? t('vazio') : loadTimeText(perf.loadedMs)}
        missing={perf.loadedMs === null}
        sub={perf.loadedMs === null ? notLoaded : comparison(perf)}
      />
      <Item
        label={t('faixa.mundo')}
        value={perf.worldMs === null ? t('vazio') : loadTimeText(perf.worldMs)}
        missing={perf.worldMs === null}
        sub={perf.worldMs === null ? t('resultado.semMundo') : null}
      />
    </section>
  );
}
