/**
 * O modo da tela do teste durante e depois da busca do culpado (SPEC T25; protótipo
 * `culpado`, `culpado~pausada`, `culpado~assistido`, `culpado~cancelada` e
 * `culpado-resultado`): `/packs/$packId/teste?modo=busca`.
 *
 * - **Em andamento:** "Buscando o mod culpado", a linha "Rodada 6 de 11 a 19 · 7 mods ainda
 *   suspeitos · 9 min até agora", a trilha de rodadas no lugar das 5 etapas, "O que o Warden
 *   está fazendo agora", a tabela das rodadas com "Ver log", os suspeitos, os sempre ligados e
 *   os desligados; "Pausar depois desta rodada" e "Cancelar busca" (com confirmação).
 * - **Modo assistido:** a rodada pergunta "Aconteceu · Não aconteceu · Não deu para testar".
 * - **Pausada:** o estado fica guardado; "Continuar a busca" (até em outro dia).
 * - **Terminada:** o resultado ([`BisectResult`]).
 */
import { useQueryClient } from '@tanstack/react-query';
import { CircleCheck, CircleMinus, CircleX, Pause, Play, Target, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { ConfirmDialog } from '../../components/common/ConfirmDialog';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Loader } from '../../components/ui/loader';
import { showToast } from '../../components/ui/toast';
import type { BisectView, PackId } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { formatDuration } from '../test/text';
import { useGame } from '../test/store';
import { bisectKeys, useAnswerBisect, useBisect, useCancelBisect, usePauseBisect } from './api';
import { BisectDialog } from './BisectDialog';
import { BisectResult } from './BisectResult';
import { RoundsTable, RoundsTrack, Suspects, trackOf } from './Rounds';
import { joinNames, namesOf, nowText, progressLine } from './text';
import { useStartBisect } from './useStartBisect';
import './bisect.css';

export function BisectPage({ packId }: { packId: PackId }) {
  const { t } = useTranslation('buscaDoCulpado');
  const bisect = useBisect(packId);
  useRefreshOnGameState(packId);
  const [dialog, setDialog] = useState(false);

  let body;
  if (bisect.isPending) {
    body = <LoadingState label={t('tela.carregando')} />;
  } else if (bisect.isError) {
    body = <ErrorPanel error={bisect.error} />;
  } else if (!bisect.data) {
    body = (
      <>
        <PageHead title={t('tela.titulo')} sans />
        <EmptyState
          title={t('tela.vazioTitulo')}
          text={t('tela.vazioTexto')}
          actions={
            <Button
              variant="primary"
              icon={Target}
              onClick={() => {
                setDialog(true);
              }}
            >
              {t('tela.encontrar')}
            </Button>
          }
        />
      </>
    );
  } else if (bisect.data.status === 'finished') {
    body = (
      <BisectResult
        packId={packId}
        view={bisect.data}
        onRestart={() => {
          setDialog(true);
        }}
      />
    );
  } else {
    body = <InProgress packId={packId} view={bisect.data} />;
  }
  return (
    <div className="stack test-page">
      {body}
      <BisectDialog packId={packId} open={dialog} onOpenChange={setDialog} />
    </div>
  );
}

/** Quando a busca começa ou termina noutro lugar (outra janela do app), a tela acompanha. */
function useRefreshOnGameState(packId: PackId) {
  const queryClient = useQueryClient();
  const game = useGame();
  const phase = game?.packId === packId ? game.state : null;
  useEffect(() => {
    void queryClient.invalidateQueries({ queryKey: bisectKeys.current(packId) });
  }, [phase, packId, queryClient]);
}

/** Relógio de 1 s para o tempo da rodada em andamento. */
function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return undefined;
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [active]);
  return now;
}

function InProgress({ packId, view }: { packId: PackId; view: BisectView }) {
  const { t } = useTranslation('buscaDoCulpado');
  const paused = view.status === 'paused';
  const pause = usePauseBisect(packId);
  const cancel = useCancelBisect(packId);
  const start = useStartBisect(packId);
  const [pauseAsked, setPauseAsked] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);

  const onPause = () => {
    pause.mutate(undefined, {
      onSuccess: () => {
        setPauseAsked(true);
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  const actions = paused ? (
    <>
      <Button
        variant="primary"
        icon={Play}
        onClick={() => {
          setPauseAsked(false);
          start({ kind: 'resume' });
        }}
      >
        {t('acoes.continuar')}
      </Button>
      <Button
        variant="danger-ghost"
        icon={X}
        onClick={() => {
          setConfirmCancel(true);
        }}
      >
        {t('acoes.cancelar')}
      </Button>
    </>
  ) : (
    <>
      <Button icon={Pause} disabled={pauseAsked} loading={pause.isPending} onClick={onPause}>
        {pauseAsked ? t('acoes.pausando') : t('acoes.pausar')}
      </Button>
      <Button
        variant="danger-ghost"
        icon={X}
        onClick={() => {
          setConfirmCancel(true);
        }}
      >
        {t('acoes.cancelar')}
      </Button>
    </>
  );

  return (
    <>
      <PageHead title={t('tela.titulo')} sub={progressLine(view)} actions={actions} sans />
      <div className="test-steps">
        <RoundsTrack entries={trackOf(view)} />
      </div>
      <div className="bisect">
        <div className="bisect__now stack">
          <Now packId={packId} view={view} pauseAsked={pauseAsked} />
          {view.inferred.map((edge) => (
            <Alert
              key={`${edge.from.path}>${edge.to.path}`}
              kind="info"
              compact
              title={t('inferida.titulo', { de: edge.from.name, para: edge.to.name })}
            >
              {t('inferida.texto', { de: edge.from.name, para: edge.to.name })}
            </Alert>
          ))}
          <h2 className="group-title bisect__rounds-title">{t('tela.rodadas')}</h2>
          <RoundsTable packId={packId} view={view} />
        </div>
        <aside className="stack-3" aria-label={t('tela.lado')}>
          <div className="panel">
            <Suspects remain={view.suspects.length} total={view.searched} />
            {view.suspects.length < view.searched ? (
              <details className="disclosure mt-2">
                <summary>{t('suspeitos.ver', { count: view.suspects.length })}</summary>
                <p className="t-sm mt-1">{joinNames(namesOf(view.suspects))}</p>
              </details>
            ) : null}
          </div>
          <div className="panel">
            <div className="t-caps t-3">{t('lado.sempre')}</div>
            <p className="t-sm t-2 mt-1">
              {view.alwaysOn.length > 0 ? joinNames(namesOf(view.alwaysOn)) : t('lado.soLoader')}
            </p>
            <div className="t-caps t-3 mt-2">{t('lado.desligados')}</div>
            <p className="t-sm t-2 mt-1">
              {view.disabled.length > 0 ? joinNames(namesOf(view.disabled)) : t('lado.nenhum')}
            </p>
          </div>
          <p className="t-xs t-3">{t('lado.explicacao')}</p>
        </aside>
      </div>
      <ConfirmDialog
        open={confirmCancel}
        onOpenChange={setConfirmCancel}
        destructive
        title={t('cancelar.titulo')}
        description={t('cancelar.texto')}
        confirmLabel={t('cancelar.confirmar')}
        confirmingLabel={t('cancelar.cancelando')}
        cancelLabel={t('cancelar.voltar')}
        onConfirm={async () => {
          await cancel.mutateAsync(undefined);
        }}
      />
    </>
  );
}

/** "O que o Warden está fazendo agora", a pergunta do modo assistido ou a pausa. */
function Now({
  packId,
  view,
  pauseAsked,
}: {
  packId: PackId;
  view: BisectView;
  pauseAsked: boolean;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  const answer = useAnswerBisect(packId);
  const current = view.current;
  const now = useNow(current !== null && !current.asking);

  if (view.status === 'paused') {
    return (
      <Alert kind="neutral" title={t('pausada.titulo')}>
        {t('pausada.texto')}
      </Alert>
    );
  }
  if (!current) {
    return (
      <div className="panel panel--selected">
        <Loader /> <span className="t-sm">{t('agora.preparando')}</span>
      </div>
    );
  }
  if (current.asking) {
    const reply = (value: 'happened' | 'notHappened' | 'couldNotTest') => {
      answer.mutate(value, {
        onError: (error) => {
          showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        },
      });
    };
    return (
      <section className="panel panel--selected" aria-labelledby="bisect-ask-title">
        <h2 className="t-caps t-3" id="bisect-ask-title">
          {t('assistido.titulo', { rodada: current.number })}
        </h2>
        <p className="mt-2">{t('assistido.texto', { count: current.enabledCount })}</p>
        <div className="btn-row mt-3">
          <Button
            variant="primary"
            icon={CircleX}
            disabled={answer.isPending}
            onClick={() => {
              reply('happened');
            }}
          >
            {t('assistido.aconteceu')}
          </Button>
          <Button
            icon={CircleCheck}
            disabled={answer.isPending}
            onClick={() => {
              reply('notHappened');
            }}
          >
            {t('assistido.naoAconteceu')}
          </Button>
          <Button
            variant="ghost"
            icon={CircleMinus}
            disabled={answer.isPending}
            onClick={() => {
              reply('couldNotTest');
            }}
          >
            {t('assistido.naoDeu')}
          </Button>
        </div>
        <p className="t-xs t-3 mt-2">{t('assistido.sozinho')}</p>
      </section>
    );
  }
  const assisted = view.symptom.kind === 'assisted';
  return (
    <section
      className="panel panel--selected"
      aria-labelledby="bisect-now-title"
      aria-live="polite"
    >
      <div className="row row--between">
        <h2 className="t-caps t-3" id="bisect-now-title">
          {t('agora.titulo', { rodada: current.number })}
        </h2>
        <span className="t-xs t-3">{formatDuration(Math.max(0, now - current.startedAtMs))}</span>
      </div>
      <p className="mt-2">{nowText(current, view.searched)}</p>
      <p className="t-xs t-3 mt-2">{assisted ? t('agora.esperaAssistido') : t('agora.espera')}</p>
      {pauseAsked ? <p className="t-xs t-2 mt-2">{t('agora.pausaPedida')}</p> : null}
    </section>
  );
}
