/**
 * A trilha de rodadas (no lugar das 5 etapas do teste), a tabela das rodadas com "Ver log" e a
 * faixa dos suspeitos (SPEC T25; protótipo `culpado` em `screens-test.js`; `W.rounds` e
 * `W.suspects` do design system). A forma do ícone diz o resultado; a cor só reforça.
 */
import {
  CircleCheck,
  CircleDashed,
  CircleMinus,
  CircleX,
  FileText,
  Hand,
  Pause,
  Timer,
  type LucideIcon,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Button } from '../../components/ui/button';
import { Dialog, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import { Loader } from '../../components/ui/loader';
import type { BisectRoundView, BisectView, PackId } from '../../lib/ipc/bindings';
import { formatDuration } from '../test/text';
import { useRoundLog } from './api';
import { RoundLogConsole } from './server/RoundLogConsole';
import { OnServerTag } from './server/ServerNotes';
import { markOf, markWord, roundWhat, verdictWord, type RoundMark } from './text';

/** As marcas da legenda, na ordem do protótipo. */
const LEGEND: readonly RoundMark[] = ['same', 'pass', 'diff', 'now', 'todo'];

/** A cor do resultado na tabela (`.status--*`). */
function statusOf(mark: RoundMark): 'danger' | 'ok' | 'warn' | 'muted' {
  switch (mark) {
    case 'same':
      return 'danger';
    case 'pass':
      return 'ok';
    case 'diff':
    case 'timeout':
      return 'warn';
    case 'now':
    case 'ask':
    case 'paused':
    case 'todo':
      return 'muted';
  }
}

const ICONS: Record<Exclude<RoundMark, 'now'>, LucideIcon> = {
  same: CircleX,
  pass: CircleCheck,
  diff: CircleMinus,
  timeout: Timer,
  ask: Hand,
  paused: Pause,
  todo: CircleDashed,
};

function Mark({ mark }: { mark: RoundMark }) {
  return (
    <span className="round__mark" aria-hidden="true">
      {mark === 'now' ? <Loader /> : <Icon icon={ICONS[mark]} />}
    </span>
  );
}

export interface TrackEntry {
  number: number;
  mark: RoundMark;
  enabled?: number;
}

/** As rodadas feitas, a atual e as que ainda faltam (até o mínimo da estimativa). */
export function trackOf(view: BisectView, withUpcoming = true): TrackEntry[] {
  const entries: TrackEntry[] = view.rounds.map((round) => ({
    number: round.number,
    mark: markOf(round.verdict),
    enabled: round.enabledCount,
  }));
  if (view.current) {
    entries.push({
      number: view.current.number,
      mark: view.current.asking ? 'ask' : 'now',
      enabled: view.current.enabledCount,
    });
  } else if (view.status === 'paused') {
    entries.push({ number: view.rounds.length, mark: 'paused' });
  }
  if (withUpcoming && view.status !== 'finished') {
    for (let number = entries.length; number < view.estimate.roundsMin; number += 1) {
      entries.push({ number, mark: 'todo' });
    }
  }
  return entries;
}

export function RoundsTrack({
  entries,
  legend = true,
}: {
  entries: readonly TrackEntry[];
  legend?: boolean;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  return (
    <>
      <ol className="rounds" aria-label={t('trilha.rotulo')}>
        {entries.map((entry) => (
          <li
            key={entry.number}
            className={`round round--${entry.mark}`}
            aria-current={entry.mark === 'now' || entry.mark === 'ask' ? 'step' : undefined}
          >
            <span className="round__n">{entry.number}</span>
            <Mark mark={entry.mark} />
            <span className="sr-only">
              {entry.enabled === undefined
                ? t('trilha.rodada', { rodada: entry.number, resultado: markWord(entry.mark) })
                : t('trilha.rodadaMods', {
                    rodada: entry.number,
                    count: entry.enabled,
                    resultado: markWord(entry.mark),
                  })}
            </span>
          </li>
        ))}
      </ol>
      {legend ? (
        <ul className="rounds-legend" aria-label={t('trilha.legenda')}>
          {LEGEND.map((mark) => (
            <li key={mark} className={`round--${mark}`}>
              <Mark mark={mark} />
              {markWord(mark)}
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}

/** A tabela das rodadas: rodada, mods ligados, resultado, tempo e "Ver log". */
export function RoundsTable({ packId, view }: { packId: PackId; view: BisectView }) {
  const { t } = useTranslation('buscaDoCulpado');
  const [log, setLog] = useState<number | null>(null);
  const found = view.outcomeItems.map((item) => item.name);
  return (
    <>
      <div className="tablewrap">
        <table className="table">
          <caption className="sr-only">{t('tabela.legenda')}</caption>
          <thead>
            <tr>
              <th className="num" scope="col">
                {t('tabela.rodada')}
              </th>
              <th scope="col">{t('tabela.ligados')}</th>
              <th scope="col">{t('tabela.resultado')}</th>
              <th scope="col">{t('tabela.tempo')}</th>
              <th className="shrink" scope="col">
                <span className="sr-only">{t('tabela.log')}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {view.rounds.map((round) => (
              <RoundRow
                key={round.number}
                round={round}
                found={found}
                onLog={() => {
                  setLog(round.number);
                }}
              />
            ))}
            {view.current ? (
              <tr>
                <td className="num">{view.current.number}</td>
                <td>
                  {roundWhat(view.current.kind, view.current.found)}{' '}
                  <span className="t-3">{t('tabela.mods', { n: view.current.enabledCount })}</span>
                </td>
                <td>
                  <span className="status status--muted">
                    {view.current.asking ? null : <Loader />}
                    {markWord(view.current.asking ? 'ask' : 'now')}
                  </span>
                </td>
                <td className="t-2" />
                <td className="shrink" />
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
      <RoundLogDialog
        packId={packId}
        round={log}
        onClose={() => {
          setLog(null);
        }}
      />
    </>
  );
}

function RoundRow({
  round,
  found,
  onLog,
}: {
  round: BisectRoundView;
  found: readonly string[];
  onLog: () => void;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  const kind = statusOf(markOf(round.verdict));
  const label = t('tabela.verLog', { rodada: round.number });
  return (
    <tr>
      <td className="num">{round.number}</td>
      <td>
        {roundWhat(round.kind, found)}{' '}
        <span className="t-3">{t('tabela.mods', { n: round.enabledCount })}</span>
      </td>
      <td>
        <span className={`status status--${kind}`}>{verdictWord(round.verdict)}</span>
        {round.repeat > 1 ? (
          <span className="t-xs t-3"> {t('tabela.repeticao', { vez: round.repeat })}</span>
        ) : null}
        {round.serverFailed ? <OnServerTag /> : null}
      </td>
      <td className="t-2">{round.durationMs > 0 ? formatDuration(round.durationMs) : null}</td>
      <td className="shrink">
        {round.hasLog || round.hasServerLog ? (
          <Button size="sm" variant="ghost" iconOnly icon={FileText} onClick={onLog}>
            {label}
          </Button>
        ) : null}
      </td>
    </tr>
  );
}

function RoundLogDialog({
  packId,
  round,
  onClose,
}: {
  packId: PackId;
  round: number | null;
  onClose: () => void;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  const log = useRoundLog(packId, round);
  const title = t('log.titulo', { rodada: round ?? 0 });
  return (
    <Dialog
      open={round !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {round !== null ? (
        <DialogContent title={title} size="lg">
          {log.isPending ? <LoadingState label={t('log.carregando')} /> : null}
          {log.isError ? <ErrorPanel error={log.error} /> : null}
          {log.isSuccess ? (
            <RoundLogConsole lines={log.data} label={title} className="bisect-log" />
          ) : null}
        </DialogContent>
      ) : null}
    </Dialog>
  );
}

/** A faixa dos suspeitos: blocos acesos = mods que ainda podem ser o culpado. */
export function Suspects({ remain, total }: { remain: number; total: number }) {
  const { t } = useTranslation('buscaDoCulpado');
  const cells = 40;
  const on = total > 0 ? Math.max(1, Math.round((remain / total) * cells)) : 0;
  const from = Math.max(0, Math.round((cells - on) / 2));
  return (
    <div className="suspects">
      <div className="suspects__bar" aria-hidden="true">
        {Array.from({ length: cells }, (_, index) => (
          <i
            key={index}
            className={index >= from && index < from + on ? 'on' : index < from ? 'out' : ''}
          />
        ))}
      </div>
      <div className="suspects__txt">{t('suspeitos.faixa', { count: remain, total })}</div>
    </div>
  );
}
