/**
 * O jogo fechou (SPEC T13 passo 7; protótipo `teste-fechou` e `teste-travou`): o resultado com a
 * duração, "Ver console" e "Testar de novo", e logo abaixo as partes registradas em
 * `result-blocks.ts` ("Por que travou"; "O que mudou durante o teste", da C-03). Serve também
 * para as sessões anteriores (console lido do `output.log`).
 */
import { CircleCheck, CircleStop, CircleX, Play, Terminal } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { ConsoleLine, PackId, TestSessionSummary } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useSaveSessionLog } from '../api';
import { Console } from '../console/Console';
import { orderedBlocks } from '../result-blocks';
import { formatDuration, formatWhen } from '../text';

const OUTCOME_ICON = {
  closedNormally: { icon: CircleCheck, className: 't-ok' },
  crashed: { icon: CircleX, className: 't-danger' },
  stoppedByUser: { icon: CircleStop, className: 't-3' },
} as const;

export interface ResultViewProps {
  packId: PackId;
  session: TestSessionSummary;
  lines: readonly ConsoleLine[];
  /** O começo do log ficou de fora (sessões com mais de 50.000 linhas). */
  truncated?: boolean;
  /** "Testar de novo" (indisponível com outro jogo aberto: o botão leva ao aviso). */
  onTestAgain: () => void;
}

export function ResultView({ packId, session, lines, truncated, onTestAgain }: ResultViewProps) {
  const { t } = useTranslation('teste');
  const save = useSaveSessionLog(packId);
  const [showConsole, setShowConsole] = useState(false);
  const outcome = OUTCOME_ICON[session.outcome];
  const game = [
    `Minecraft ${session.minecraft}`,
    session.loader ? `${session.loader} ${session.loaderVersion ?? ''}`.trim() : null,
  ]
    .filter(Boolean)
    .join(' · ');
  const sub = [
    t('resultado.sub', {
      quando: formatWhen(session.startedAtMs),
      duracao: formatDuration(session.durationMs),
    }),
    t('resultado.jogoDe', { jogo: game, java: session.javaMajor }),
    session.packVersion ? t('resultado.versaoDoPack', { versao: session.packVersion }) : null,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <>
      <PageHead
        sans
        title={
          <span className="result-title">
            <Icon icon={outcome.icon} className={`icon--lg ${outcome.className}`} />
            {t(`resultado.${session.outcome}`)}
          </span>
        }
        sub={sub}
        actions={
          <>
            <Button
              variant="ghost"
              icon={Terminal}
              aria-expanded={showConsole}
              onClick={() => {
                setShowConsole((open) => !open);
              }}
            >
              {showConsole ? t('resultado.ocultarConsole') : t('resultado.verConsole')}
            </Button>
            <Button icon={Play} onClick={onTestAgain}>
              {t('resultado.testarDeNovo')}
            </Button>
          </>
        }
      />
      <div className="stack test-result">
        {orderedBlocks().map(({ id, component: Block }) => (
          <Block key={id} packId={packId} session={session} lines={lines} />
        ))}
        {showConsole ? (
          <div className="stack-2">
            {truncated ? (
              <Alert kind="neutral" compact>
                {t('sessao.cortado')}
              </Alert>
            ) : null}
            <div className="result-console">
              <Console
                lines={lines}
                state="ended"
                onSave={() => {
                  save.mutate(session.id, {
                    onSuccess: (path) => {
                      if (path) {
                        showToast({ kind: 'ok', title: t('console.salvo', { caminho: path }) });
                      }
                    },
                    onError: (error) => {
                      showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
                    },
                  });
                }}
              />
            </div>
          </div>
        ) : null}
      </div>
    </>
  );
}
