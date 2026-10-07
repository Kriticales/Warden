/**
 * Tela do teste (SPEC T13): não é seção do menu do pack; abre pelo botão Testar do cabeçalho,
 * por "Ver último teste" do ▾ e pelos testes anteriores (`?sessao=<id>`).
 *
 * O que aparece, nesta ordem de prioridade:
 * 1. o teste em andamento deste pack (etapas e preparação, ou o jogo aberto com o console);
 * 2. a sessão pedida em `?sessao=`;
 * 3. o teste que acabou de terminar (com o console ao vivo, que inclui as linhas do Warden), ou
 *    o motivo de ele não ter começado;
 * 4. a sessão mais nova gravada;
 * 5. "Este pack ainda não foi testado".
 * Abaixo do resultado ficam os testes anteriores.
 */
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { FolderOpen, Play, RefreshCw, Terminal } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { showToast } from '../../components/ui/toast';
import type { AppError, PackId, PackRow } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { usePack } from '../packs/api';
import {
  invalidateAfterTest,
  useAdoptedConsole,
  useRevealInstance,
  useTestSession,
  useTestSessions,
} from './api';
import {
  DEFAULT_REQUEST,
  isActive,
  useGame,
  useTestRun,
  useTestStore,
  type TestRun,
} from './store';
import { PrepareView, RunningView } from './views/LiveViews';
import { ResultView } from './views/ResultView';
import { SessionsTable } from './views/SessionsTable';
import './test.css';

export function TestPage({ packId, sessionId }: { packId: PackId; sessionId: string | null }) {
  const { t } = useTranslation('teste');
  const pack = usePack(packId);
  const run = useTestRun(packId);
  useAdoptedConsole(packId);
  useInvalidateWhenDone(packId, run);

  if (!pack.isSuccess) {
    return pack.isError ? (
      <ErrorPanel error={pack.error} />
    ) : (
      <LoadingState label={t('sessao.carregando')} />
    );
  }
  if (run && isActive(run)) {
    return (
      <div className="stack test-page">
        {run.status === 'running' ? (
          <RunningView run={run} pack={pack.data} />
        ) : (
          <PrepareView run={run} pack={pack.data} />
        )}
      </div>
    );
  }
  return (
    <div className="stack test-page">
      <Finished packId={packId} pack={pack.data} run={run} sessionId={sessionId} />
    </div>
  );
}

/** Quando o teste deste pack termina, a lista de sessões e o "último teste" mudam. */
function useInvalidateWhenDone(packId: PackId, run: TestRun | undefined) {
  const queryClient = useQueryClient();
  const status = run?.status;
  const previous = useRef(status);
  useEffect(() => {
    const was = previous.current;
    previous.current = status;
    if ((was === 'preparing' || was === 'running') && status !== was) {
      void invalidateAfterTest(queryClient, packId);
    }
  }, [status, packId, queryClient]);
}

/** Começa um teste do pack ou avisa que outro jogo está aberto. */
function useTestAgain(packId: PackId) {
  const start = useTestStore((store) => store.start);
  const game = useGame();
  const navigate = useNavigate();
  const { t } = useTranslation('teste');
  return (request = DEFAULT_REQUEST) => {
    if (game && game.packId !== packId) {
      showToast({ kind: 'warn', title: t('botao.outroJogo', { pack: game.packName }) });
      return;
    }
    void navigate({ to: '/packs/$packId/teste', params: { packId }, search: {} });
    void start(packId, request);
  };
}

function Finished({
  packId,
  pack,
  run,
  sessionId,
}: {
  packId: PackId;
  pack: PackRow;
  run: TestRun | undefined;
  sessionId: string | null;
}) {
  const { t } = useTranslation('teste');
  const testAgain = useTestAgain(packId);
  const sessions = useTestSessions(packId);
  const fromRun = !sessionId && run?.status === 'finished' && run.result ? run : null;
  const wantedId =
    sessionId ?? (fromRun || run?.status === 'failed' ? null : (sessions.data?.[0]?.id ?? null));
  const stored = useTestSession(packId, fromRun ? null : wantedId);

  let body;
  if (fromRun?.result) {
    body = (
      <ResultView
        packId={packId}
        session={fromRun.result}
        lines={fromRun.lines}
        onTestAgain={() => {
          testAgain();
        }}
      />
    );
  } else if (!sessionId && run?.status === 'failed' && run.error) {
    body = <FailedView packId={packId} run={run} error={run.error} onRetry={testAgain} />;
  } else if (wantedId === null) {
    body = sessions.isPending ? (
      <LoadingState label={t('sessoes.carregando')} />
    ) : (
      <EmptyState
        title={t('nenhum.titulo')}
        text={t('nenhum.texto')}
        headingLevel={2}
        actions={
          <Button
            variant="primary"
            icon={Play}
            onClick={() => {
              testAgain();
            }}
          >
            {t('botao.testar')}
          </Button>
        }
      />
    );
  } else if (stored.isPending) {
    body = <LoadingState label={t('sessao.carregando')} />;
  } else if (stored.isError) {
    body = <ErrorPanel title={t('sessao.erro')} error={stored.error} />;
  } else {
    body = (
      <ResultView
        packId={packId}
        session={stored.data.summary}
        lines={stored.data.lines}
        truncated={stored.data.truncated}
        onTestAgain={() => {
          testAgain();
        }}
      />
    );
  }

  const currentId = fromRun?.result?.id ?? wantedId;
  return (
    <>
      {run?.status === 'cancelled' && !sessionId ? (
        <Alert kind="neutral" compact>
          {t('preparo.cancelado')}
        </Alert>
      ) : null}
      {body}
      {pack.status === 'ready' ? <SessionsTable packId={packId} currentId={currentId} /> : null}
    </>
  );
}

/** O teste não começou: o motivo e o que fazer. */
function FailedView({
  packId,
  run,
  error,
  onRetry,
}: {
  packId: PackId;
  run: TestRun;
  error: AppError;
  onRetry: (request?: typeof DEFAULT_REQUEST) => void;
}) {
  const { t } = useTranslation('teste');
  const reveal = useRevealInstance(packId);
  const navigate = useNavigate();
  const code = `${error.code.domain}.${error.code.code}`;

  if (code === 'app.TEST_INSTANCE_CHANGED') {
    const count = Number(error.params.count ?? '0');
    const files = (error.params.files ?? '').split('\n').filter(Boolean);
    return (
      <>
        <PageHead title={t('preparo.falhou')} />
        <section className="panel panel--strong stack-3" aria-labelledby="instance-changed-title">
          <h2 className="panel__title" id="instance-changed-title">
            {t('instanciaMudou.titulo', { count })}
          </h2>
          <p>{t('instanciaMudou.texto')}</p>
          <ul className="changed-files">
            {files.map((file) => (
              <li key={file} className="path">
                {file}
              </li>
            ))}
          </ul>
          <div className="btn-row">
            <Button
              variant="primary"
              icon={RefreshCw}
              onClick={() => {
                onRetry({ ...run.request, replaceInstanceChanges: true });
              }}
            >
              {t('instanciaMudou.substituir')}
            </Button>
            <Button
              icon={FolderOpen}
              onClick={() => {
                reveal.mutate(undefined, {
                  onError: (cause) => {
                    showToast({ kind: 'danger', title: appErrorMessage(toAppError(cause)) });
                  },
                });
              }}
            >
              {t('instanciaMudou.abrirPasta')}
            </Button>
          </div>
        </section>
      </>
    );
  }

  const otherGame = code === 'app.GAME_ALREADY_RUNNING' ? (error.params.packId ?? null) : null;
  return (
    <>
      <PageHead title={t('preparo.falhou')} />
      <ErrorPanel
        error={error}
        {...(otherGame
          ? {}
          : {
              onRetry: () => {
                onRetry(run.request);
              },
            })}
        actions={
          otherGame ? (
            <Button
              variant="primary"
              icon={Terminal}
              onClick={() => {
                void navigate({
                  to: '/packs/$packId/teste',
                  params: { packId: otherGame },
                  search: {},
                });
              }}
            >
              {t('outroJogo.irParaTeste')}
            </Button>
          ) : undefined
        }
      />
    </>
  );
}
