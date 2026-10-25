/**
 * "Desempenho do último teste" na coluna da direita da seção Problemas (SPEC T14; esboço
 * aprovado do bug-01, "Separadas"): do último teste que chegou a carregar, o tempo para abrir
 * com a variação para a última versão testada, até entrar no mundo, a memória máxima (com "no
 * limite" quando o jogo ficou sem folga, `W_LOW_HEAP`) e o tempo por tick do spark quando o
 * teste salvou um perfil; o aviso de versão mais pesada quando o critério da T33 se cumpre; os
 * mods que mais usaram processamento no perfil do spark; e **✦ Perguntar ao Sculk por que
 * ficou mais lento**. Registrado em `problems-blocks.ts` (coluna `side`).
 */
import { Link } from '@tanstack/react-router';
import { ArrowRight, Gauge } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Icon } from '../../../components/ui/icon';
import type { LastTestPerf, PackId, SparkRow } from '../../../lib/ipc/bindings';
import { loadTimeText, memoryText } from '../../test/perf-strip/format';
import { formatWhen } from '../../test/text';
import { AskSculkLink } from '../components/AskSculk';
import type { ProblemsBlockProps } from '../problems-blocks';
import { useLastTestPerf } from './api';
import {
  gotSlower,
  heavierText,
  loadDelta,
  loadDeltaText,
  memoryOfText,
  sculkSubject,
  tickText,
} from './text';

/** Mods mostrados em "Mods que mais usaram processamento". */
const TOP_MODS = 5;

const PERCENT = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 0 });

export function PerfBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('desempenho');
  const titleId = useId();
  const perf = useLastTestPerf(packId);
  const data = perf.data;

  let body;
  if (perf.isPending) {
    body = <LoadingState label={t('ultimoTeste.carregando')} inline />;
  } else if (perf.isError) {
    body = (
      <ErrorPanel
        title={t('ultimoTeste.erro')}
        error={perf.error}
        compact
        onRetry={() => {
          void perf.refetch();
        }}
      />
    );
  } else if (!data) {
    body = (
      <>
        <p className="t-sm">{t('ultimoTeste.vazio')}</p>
        <p className="t-xs t-3">{t('ultimoTeste.vazioTexto')}</p>
      </>
    );
  } else {
    body = <Measures perf={data} />;
  }

  return (
    <section className="panel panel--parts" aria-labelledby={titleId}>
      <div className="panel__bar">
        <h2 className="panel__bar-title" id={titleId}>
          <Icon icon={Gauge} />
          {t('ultimoTeste.titulo')}
        </h2>
        {data ? <span className="t-xs t-3">{formatWhen(data.startedAtMs)}</span> : null}
      </div>
      <div className="panel__body">{body}</div>
      {data ? <Foot packId={packId} perf={data} /> : null}
    </section>
  );
}

function Measures({ perf }: { perf: LastTestPerf }) {
  const { t } = useTranslation('desempenho');
  const delta = perf.previous ? loadDelta(perf.loadedMs, perf.previous) : null;
  const aviso = perf.comparison ? heavierText(perf.comparison) : null;
  return (
    <>
      <dl className="metrics">
        <div>
          <dt>{t('ultimoTeste.abrir')}</dt>
          <dd>
            {loadTimeText(perf.loadedMs)}
            {perf.previous ? (
              <small
                className={`metrics__note${delta !== null && delta > 0 ? ' metrics__note--warn' : ''}`}
              >
                {loadDeltaText(perf.loadedMs, perf.previous)}
              </small>
            ) : null}
          </dd>
        </div>
        <div>
          <dt>{t('ultimoTeste.mundo')}</dt>
          <dd>
            {perf.worldMs === null ? (
              <span className="t-3">{t('ultimoTeste.semMundo')}</span>
            ) : (
              loadTimeText(perf.worldMs)
            )}
          </dd>
        </div>
        {perf.memory ? (
          <div>
            <dt>{t('ultimoTeste.memoria')}</dt>
            <dd>
              {memoryOfText(perf.memory.usedMb, perf.memory.maxMb)}
              {perf.memory.nearLimit ? (
                <small className="metrics__note metrics__note--warn">
                  {t('ultimoTeste.noLimite')}
                </small>
              ) : null}
            </dd>
          </div>
        ) : perf.ramPeakMb !== null ? (
          <div>
            <dt>{t('ultimoTeste.memoria')}</dt>
            <dd>
              {memoryText(perf.ramPeakMb)}
              <small className="metrics__note">{t('ultimoTeste.ramProcesso')}</small>
            </dd>
          </div>
        ) : null}
        {perf.tick ? (
          <div>
            <dt>{t('ultimoTeste.tick')}</dt>
            <dd>{tickText(perf.tick.msptMedian)}</dd>
          </div>
        ) : null}
      </dl>
      {aviso ? (
        <Alert kind="warn" compact>
          {aviso}
        </Alert>
      ) : perf.comparison?.status === 'fewTests' ? (
        <p className="t-xs t-3">
          {t('ultimoTeste.poucosTestes', { anterior: perf.comparison.previousVersion })}
        </p>
      ) : null}
      <div className="stack-2">
        <h3 className="panel__subtitle">{t('ultimoTeste.sparkTitulo')}</h3>
        {perf.sparkTop && perf.sparkTop.length > 0 ? (
          <HeavyMods rows={perf.sparkTop} />
        ) : (
          <p className="t-xs t-3">{t('ultimoTeste.semSpark')}</p>
        )}
      </div>
    </>
  );
}

function HeavyMods({ rows }: { rows: SparkRow[] }) {
  const top = rows.slice(0, TOP_MODS);
  const max = Math.max(...top.map((row) => row.percent), 1);
  return (
    <ul className="barlist">
      {top.map((row) => {
        const percentual = `${PERCENT.format(row.percent)}%`;
        return (
          <li key={row.item ?? row.modId ?? row.name}>
            <span className="barlist__name" title={row.name}>
              {row.name}
            </span>
            <span className="barlist__bar" aria-hidden="true">
              <i style={{ width: `${String(Math.round((row.percent / max) * 100))}%` }} />
            </span>
            <span className="barlist__pct">{percentual}</span>
          </li>
        );
      })}
    </ul>
  );
}

function Foot({ packId, perf }: { packId: PackId; perf: LastTestPerf }) {
  const { t } = useTranslation('desempenho');
  return (
    <div className="panel__foot">
      <Link
        to="/packs/$packId/teste"
        params={{ packId }}
        search={{ sessao: perf.sessionId }}
        className="link diag-perf__link"
      >
        {t('ultimoTeste.verTeste')}
        <Icon icon={ArrowRight} size="sm" />
      </Link>
      {gotSlower(perf) ? (
        <AskSculkLink
          packId={packId}
          subject={{ sessao: perf.sessionId, sintoma: 'slow', assunto: sculkSubject(perf) }}
        >
          {t('ultimoTeste.perguntar')}
        </AskSculkLink>
      ) : null}
    </div>
  );
}
