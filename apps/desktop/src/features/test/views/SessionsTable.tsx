/**
 * "Testes anteriores" (SPEC T13 "sessões anteriores"; ARCHITECTURE §7.4): as sessões gravadas
 * do pack, da mais nova para a mais antiga. Cada linha abre o resultado e o console daquela
 * sessão.
 */
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { cn } from '../../../lib/cn';
import type { PackId, TestSessionSummary } from '../../../lib/ipc/bindings';
import { useTestSessions } from '../api';
import { formatDuration, formatWhen, outcomeShort } from '../text';

const STATUS: Record<TestSessionSummary['outcome'], string> = {
  closedNormally: 'ok',
  crashed: 'danger',
  stoppedByUser: 'muted',
};

export function SessionsTable({
  packId,
  currentId,
}: {
  packId: PackId;
  /** A sessão mostrada agora (marcada na lista). */
  currentId: string | null;
}) {
  const { t } = useTranslation('teste');
  const sessions = useTestSessions(packId);
  return (
    <section className="stack-2" aria-labelledby="test-sessions-title">
      <h2 className="group-title" id="test-sessions-title">
        {t('sessoes.titulo')}
      </h2>
      {sessions.isPending ? (
        <LoadingState label={t('sessoes.carregando')} />
      ) : sessions.isError ? (
        <ErrorPanel
          title={t('sessoes.erro')}
          error={sessions.error}
          compact
          onRetry={() => {
            void sessions.refetch();
          }}
        />
      ) : sessions.data.length === 0 ? (
        <p className="t-sm t-3">{t('sessoes.vazio')}</p>
      ) : (
        <>
          <div className="tablewrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t('sessoes.quando')}</th>
                  <th>{t('sessoes.resultado')}</th>
                  <th>{t('sessoes.duracao')}</th>
                  <th>{t('sessoes.jogo')}</th>
                  <th>{t('sessoes.versao')}</th>
                </tr>
              </thead>
              <tbody>
                {sessions.data.map((session) => {
                  const when = formatWhen(session.startedAtMs);
                  const current = session.id === currentId;
                  return (
                    <tr key={session.id} className={cn(current && 'is-selected')}>
                      <td>
                        <Link
                          to="/packs/$packId/teste"
                          params={{ packId }}
                          search={{ sessao: session.id }}
                          aria-label={t('sessoes.abrir', { quando: when })}
                          aria-current={current ? 'page' : undefined}
                        >
                          {when}
                        </Link>
                        {current ? <span className="t-xs t-3"> · {t('sessoes.atual')}</span> : null}
                      </td>
                      <td>
                        <span className={`status status--${STATUS[session.outcome]}`}>
                          {outcomeShort(session.outcome)}
                        </span>
                      </td>
                      <td className="t-2">{formatDuration(session.durationMs)}</td>
                      <td className="t-2">
                        {[session.minecraft, session.loader].filter(Boolean).join(' · ')}
                      </td>
                      <td className="t-2">{session.packVersion ?? '—'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="t-xs t-3">{t('sessoes.limite')}</p>
        </>
      )}
    </section>
  );
}
