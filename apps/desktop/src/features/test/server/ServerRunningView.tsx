/**
 * Tela do teste com o servidor local aberto (SPEC T27 passo 3; protótipo `teste-servidor`):
 * "Testando como servidor" com a porta, o indicador de etapas, o console do servidor separado
 * do console do jogo ("Mostrando: Servidor · Jogo", caixa de seleção, não aba), a linha de
 * comando do servidor ("Enviar comando ao servidor") e "Parar jogo e servidor", que encerra os
 * dois (o servidor recebe `stop` e, se não fechar em 60 s, é encerrado à força).
 */
import { Send, Square } from 'lucide-react';
import { useId, useMemo, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { formatTime } from '../../../lib/format';
import type { ConsoleLine, PackRow } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useSaveSessionLog } from '../api';
import { Console } from '../console/Console';
import { LivePerfStrip } from '../perf-strip/PerfStrip';
import { useGame, useTestStore, type TestRun } from '../store';
import { TestSteps, gameLine } from '../views/LiveViews';
import { useSendServerCommand, useServerState } from './api';
import './server.css';

/** Se a linha é do servidor (a saída dele ou uma mensagem do Warden sobre ele). */
export function isServerLine(line: ConsoleLine): boolean {
  return (
    line.origin === 'server' || (line.origin === 'warden' && line.text.startsWith('servidor:'))
  );
}

/** As linhas do servidor e as do jogo, separadas. */
export function splitLines(lines: readonly ConsoleLine[]): {
  server: ConsoleLine[];
  game: ConsoleLine[];
} {
  const server: ConsoleLine[] = [];
  const game: ConsoleLine[] = [];
  for (const line of lines) {
    (isServerLine(line) ? server : game).push(line);
  }
  return { server, game };
}

type Shown = 'server' | 'game';

/** As opções de "Mostrando": valor e texto do catálogo. */
const SHOWN_OPTIONS = [
  ['server', 'jogo.servidor'],
  ['game', 'jogo.jogo'],
] as const satisfies readonly (readonly [Shown, string])[];

export function ServerRunningView({ run, pack }: { run: TestRun; pack: PackRow }) {
  const { t } = useTranslation('servidor');
  const { t: tt } = useTranslation('teste');
  const stop = useTestStore((store) => store.stop);
  const game = useGame();
  const server = useServerState();
  const save = useSaveSessionLog(pack.id);
  const [confirmStop, setConfirmStop] = useState(false);
  const [shown, setShown] = useState<Shown>('server');
  const only = run.request.serverOnly === true;
  const selectId = useId();
  const { server: serverLines, game: gameLines } = useMemo(
    () => splitLines(run.lines),
    [run.lines],
  );
  const sessionId = game?.packId === pack.id ? game.sessionId : null;
  const port = server?.packId === pack.id ? server.port : null;
  const ready = server?.packId === pack.id && server.phase === 'ready';
  const joined = serverLines.find(
    (line) => line.origin === 'server' && line.text.includes('joined the game'),
  );

  const subParts = [
    port === null ? t('jogo.subAbrindo') : t('jogo.sub', { porta: port }),
    gameLine(pack),
    joined?.atMs ? t('jogo.subEntrou', { hora: formatTime(joined.atMs) }) : null,
  ];
  const showServer = only || shown === 'server';
  const onError = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };
  // O `output.log` da sessão é o do jogo (com o jogo) ou o do servidor (em "Só o servidor").
  const savesShown = sessionId !== null && (only || !showServer);

  return (
    <>
      <PageHead
        sans
        title={t('jogo.titulo')}
        sub={subParts.filter(Boolean).join(' · ')}
        actions={
          <Button
            variant="danger-ghost"
            icon={Square}
            loading={run.stopping}
            onClick={() => {
              setConfirmStop(true);
            }}
          >
            {run.stopping ? t('jogo.parando') : only ? t('jogo.pararSo') : t('jogo.parar')}
          </Button>
        }
      />
      <TestSteps run={run} />
      <div className="stack-3">
        {only ? null : <LivePerfStrip run={run} />}
        {run.warnings.map((warning, index) => (
          <Alert key={index} kind="warn" compact>
            {appErrorMessage(warning)}
          </Alert>
        ))}
        {only ? null : (
          <div className="row server-console__show">
            <label className="t-sm t-2" htmlFor={selectId}>
              {t('jogo.mostrando')}
            </label>
            <select
              id={selectId}
              className="select select--sm"
              value={shown}
              onChange={(event) => {
                setShown(event.target.value as Shown);
              }}
            >
              {SHOWN_OPTIONS.map(([value, key]) => (
                <option key={value} value={value}>
                  {t(key)}
                </option>
              ))}
            </select>
          </div>
        )}
        <div className="run-console">
          <Console
            key={showServer ? 'server' : 'game'}
            lines={showServer ? serverLines : gameLines}
            state="live"
            label={showServer ? t('jogo.consoleServidor') : t('jogo.consoleJogo')}
            onSave={
              savesShown
                ? () => {
                    save.mutate(sessionId, {
                      onSuccess: (path) => {
                        if (path) {
                          showToast({ kind: 'ok', title: tt('console.salvo', { caminho: path }) });
                        }
                      },
                      onError,
                    });
                  }
                : undefined
            }
          />
        </div>
        {showServer ? <CommandLine packId={pack.id} ready={ready} /> : null}
      </div>
      <ConfirmDialog
        open={confirmStop}
        onOpenChange={setConfirmStop}
        title={only ? t('jogo.pararTituloSo') : t('jogo.pararTitulo')}
        description={only ? t('jogo.pararTextoSo') : t('jogo.pararTexto')}
        confirmLabel={only ? t('jogo.pararSo') : t('jogo.parar')}
        confirmingLabel={t('jogo.parando')}
        cancelLabel={t('jogo.continuar')}
        onConfirm={async () => {
          await stop(run.packId);
        }}
      />
    </>
  );
}

/** "Enviar comando ao servidor" (stdin). */
function CommandLine({ packId, ready }: { packId: PackRow['id']; ready: boolean }) {
  const { t } = useTranslation('servidor');
  const send = useSendServerCommand(packId);
  const [command, setCommand] = useState('');
  const inputId = useId();
  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    const text = command.trim();
    if (!text) return;
    send.mutate(text, {
      onSuccess: () => {
        setCommand('');
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };
  return (
    <form className="row server-console__command" onSubmit={submit}>
      <label className="sr-only" htmlFor={inputId}>
        {t('jogo.comando')}
      </label>
      <input
        id={inputId}
        className="input input--sm grow t-mono"
        value={command}
        placeholder={ready ? t('jogo.comandoExemplo') : t('jogo.comandoEsperando')}
        disabled={!ready}
        autoComplete="off"
        spellCheck={false}
        maxLength={256}
        onChange={(event) => {
          setCommand(event.target.value);
        }}
      />
      <Button
        type="submit"
        size="sm"
        icon={Send}
        disabled={!ready || command.trim() === ''}
        loading={send.isPending}
      >
        {t('jogo.enviar')}
      </Button>
    </form>
  );
}
