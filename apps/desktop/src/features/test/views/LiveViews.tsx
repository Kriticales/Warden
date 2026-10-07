/**
 * A tela do teste em andamento (SPEC T13 passos 2 a 6; protótipo `teste-preparo` e
 * `teste-jogo`): o indicador de etapas, a preparação com o progresso e o painel "Este teste",
 * e o jogo aberto com o console ao vivo, "Parar jogo" (com confirmação) e "Abrir pasta da
 * instância".
 */
import { useQuery } from '@tanstack/react-query';
import { FolderOpen, Square, X } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Steps } from '../../../components/common/Steps';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { formatTime } from '../../../lib/format';
import { commands, type PackRow } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { commandQuery } from '../../../lib/ipc/query';
import { progressText } from '../../../app/tasks/operation-text';
import { editorKeys, useTestSettings } from '../../pack-editor/api';
import { javaRequest } from '../../pack-editor/test-settings/TestSettingsDialog';
import { loaderName } from '../../packs/lib/pack-list';
import { useSettings } from '../../settings/api';
import { useRevealInstance, useSaveSessionLog } from '../api';
import { Console } from '../console/Console';
import { currentStep, orderedSteps, stepLabel } from '../steps';
import { useGame, useTestStore, type TestRun } from '../store';
import { formatMemory, stageText } from '../text';

/** O jogo do pack em uma linha: "Minecraft 1.20.1 com Fabric 0.19.5". */
export function gameLine(pack: PackRow): string {
  const minecraft = `Minecraft ${pack.minecraft ?? '?'}`;
  return pack.loader
    ? `${minecraft} com ${loaderName(pack.loader)} ${pack.loaderVersion ?? ''}`.trim()
    : minecraft;
}

/** O indicador de etapas. */
export function TestSteps({ run }: { run: TestRun }) {
  const { t } = useTranslation('teste');
  const steps = orderedSteps();
  const current = currentStep(run.stagesSeen, run.status === 'running');
  return (
    <div className="test-steps">
      <Steps items={steps.map(stepLabel)} current={current} label={t('etapas.rotulo')} />
    </div>
  );
}

/** Avisos que não interromperam o teste (memória alta no Java 8…). */
function Warnings({ run }: { run: TestRun }) {
  if (run.warnings.length === 0) {
    return null;
  }
  return (
    <>
      {run.warnings.map((warning, index) => (
        <Alert key={index} kind="warn" compact>
          {appErrorMessage(warning)}
        </Alert>
      ))}
    </>
  );
}

/** Passos 2 a 5: preparando. */
export function PrepareView({ run, pack }: { run: TestRun; pack: PackRow }) {
  const { t } = useTranslation('teste');
  const stop = useTestStore((store) => store.stop);
  const steps = orderedSteps();
  const current = currentStep(run.stagesSeen, false);
  const progress = run.progress;
  const percent = progress?.total ? Math.min(100, (progress.current / progress.total) * 100) : null;
  const label = (run.stage ? stageText(run.stage.labelKey) : null) ?? t('preparo.aguarde');
  return (
    <>
      <PageHead
        title={t('preparo.titulo')}
        sub={t('preparo.sub', {
          atual: Math.min(current + 1, steps.length),
          total: steps.length,
          hora: formatTime(run.startedAtMs),
        })}
        actions={
          <Button
            variant="ghost"
            icon={X}
            loading={run.stopping}
            onClick={() => {
              void stop(run.packId);
            }}
          >
            {run.stopping ? t('preparo.cancelando') : t('preparo.cancelar')}
          </Button>
        }
      />
      <TestSteps run={run} />
      <div className="prep">
        <div className="panel panel--strong stack">
          <ProgressBar
            value={percent}
            label={label}
            meta={progress ? progressText(progress) : undefined}
          />
          <Warnings run={run} />
          <p className="t-sm t-3">{t('preparo.primeiraVez')}</p>
        </div>
        <ThisTest pack={pack} />
      </div>
    </>
  );
}

/** O painel "Este teste": jogo, Java, memória e jogador. */
function ThisTest({ pack }: { pack: PackRow }) {
  const { t } = useTranslation('teste');
  const settings = useTestSettings(pack.id);
  const app = useSettings();
  const userJava = settings.data?.settings.java ?? null;
  const choice = useQuery({
    ...commandQuery([...editorKeys.testSettings(pack.id), 'java', userJava ?? 'auto'], () =>
      commands.javaChoice(javaRequest(pack, userJava)),
    ),
    enabled: settings.isSuccess,
  });
  const memory = settings.data?.settings.memory;
  return (
    <aside className="panel" aria-labelledby="this-test-title">
      <div className="panel__title panel__title--sans" id="this-test-title">
        {t('esteTeste.titulo')}
      </div>
      <dl className="kv">
        <dt>{t('esteTeste.minecraft')}</dt>
        <dd>{pack.minecraft ?? '?'}</dd>
        {pack.loader ? (
          <>
            <dt>{t('esteTeste.loader')}</dt>
            <dd>{`${loaderName(pack.loader)} ${pack.loaderVersion ?? ''}`.trim()}</dd>
          </>
        ) : null}
        <dt>{t('esteTeste.java')}</dt>
        <dd>
          {choice.data
            ? userJava
              ? t('esteTeste.javaEscolhido', { major: choice.data.major })
              : t('esteTeste.javaAuto', { major: choice.data.major })
            : '…'}
        </dd>
        <dt>{t('esteTeste.memoria')}</dt>
        <dd>{memory?.mode === 'fixed' ? formatMemory(memory.mb) : t('esteTeste.memoriaAuto')}</dd>
        <dt>{t('esteTeste.jogador')}</dt>
        <dd>
          {app.data ? t('esteTeste.jogadorOffline', { nome: app.data.playerName ?? '' }) : '…'}
        </dd>
      </dl>
    </aside>
  );
}

/** Passo 6: o jogo aberto. */
export function RunningView({ run, pack }: { run: TestRun; pack: PackRow }) {
  const { t } = useTranslation('teste');
  const stop = useTestStore((store) => store.stop);
  const game = useGame();
  const reveal = useRevealInstance(pack.id);
  const save = useSaveSessionLog(pack.id);
  const [confirmStop, setConfirmStop] = useState(false);
  const sessionId = game?.packId === pack.id ? game.sessionId : null;
  const onError = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };
  return (
    <>
      <PageHead
        title={t('jogo.titulo')}
        sub={t('jogo.sub', { hora: formatTime(run.startedAtMs), jogo: gameLine(pack) })}
        actions={
          <>
            <Button
              variant="ghost"
              icon={FolderOpen}
              iconOnly
              onClick={() => {
                reveal.mutate(undefined, { onError });
              }}
            >
              {t('jogo.abrirPasta')}
            </Button>
            <Button
              variant="danger-ghost"
              icon={Square}
              loading={run.stopping}
              onClick={() => {
                setConfirmStop(true);
              }}
            >
              {run.stopping ? t('jogo.parando') : t('jogo.parar')}
            </Button>
          </>
        }
      />
      <TestSteps run={run} />
      <div className="stack-3">
        <Warnings run={run} />
        <div className="run-console">
          <Console
            lines={run.lines}
            state="live"
            onSave={
              sessionId
                ? () => {
                    save.mutate(sessionId, {
                      onSuccess: (path) => {
                        if (path)
                          showToast({ kind: 'ok', title: t('console.salvo', { caminho: path }) });
                      },
                      onError,
                    });
                  }
                : undefined
            }
          />
        </div>
      </div>
      <ConfirmDialog
        open={confirmStop}
        onOpenChange={setConfirmStop}
        title={t('jogo.pararTitulo')}
        description={t('jogo.pararTexto')}
        confirmLabel={t('jogo.parar')}
        confirmingLabel={t('jogo.parando')}
        cancelLabel={t('jogo.continuar')}
        onConfirm={async () => {
          await stop(run.packId);
        }}
      >
        <p className="t-sm t-3">{t('jogo.pararDica')}</p>
      </ConfirmDialog>
    </>
  );
}
