/**
 * "Por que travou" no resultado de um teste que travou (SPEC T13 passo 7; CA-T13-06). Esta é a
 * parte do Testar: o código de saída, os arquivos de travamento para abrir e as últimas linhas
 * com erro do console. A análise da causa com a correção sugerida é da D-03, que troca esta
 * linha em `result-blocks.ts`.
 */
import { FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useOpenArtifact } from '../api';
import { Console } from '../console/Console';
import type { ResultBlockProps } from '../result-blocks';
import { formatDuration } from '../text';

/** Quantas linhas com erro aparecem. */
const LAST_ERRORS = 12;

export function WhyCrashed({ packId, session, lines }: ResultBlockProps) {
  const { t } = useTranslation('teste');
  const open = useOpenArtifact(packId);
  if (session.outcome !== 'crashed') {
    return null;
  }
  const duracao = formatDuration(session.durationMs);
  const errors = lines
    .filter((line) => line.level === 'error' || line.level === 'fatal')
    .slice(-LAST_ERRORS);
  const openFile = (path: string) => {
    open.mutate(
      { sessionId: session.id, path },
      {
        onError: (error) => {
          showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        },
      },
    );
  };
  return (
    <section className="panel panel--strong stack-3" aria-labelledby="why-crashed-title">
      <h2 className="panel__title" id="why-crashed-title">
        {t('porQueTravou.titulo')}
      </h2>
      <p>
        {session.exitCode === null
          ? t('porQueTravou.semCodigo', { duracao })
          : t('porQueTravou.codigo', { codigo: session.exitCode, duracao })}
      </p>
      {session.crashReports.length > 0 ? (
        <p className="t-sm t-2">{t('porQueTravou.comRelatorio')}</p>
      ) : null}
      {session.hsErrFiles.length > 0 ? (
        <p className="t-sm t-2">{t('porQueTravou.comErroJava')}</p>
      ) : null}
      {session.crashReports.length > 0 || session.hsErrFiles.length > 0 ? (
        <div className="btn-row">
          {session.crashReports.map((path) => (
            <Button
              key={path}
              size="sm"
              variant="primary"
              icon={FileText}
              onClick={() => {
                openFile(path);
              }}
            >
              {t('resultado.abrirCrash')}
            </Button>
          ))}
          {session.hsErrFiles.map((path) => (
            <Button
              key={path}
              size="sm"
              icon={FileText}
              onClick={() => {
                openFile(path);
              }}
            >
              {t('resultado.abrirErroJava')}
            </Button>
          ))}
        </div>
      ) : null}
      <div className="stack-2">
        <h3 className="field__label">{t('porQueTravou.ultimasLinhas')}</h3>
        {errors.length > 0 ? (
          <Console
            lines={errors}
            state="ended"
            label={t('porQueTravou.ultimasLinhas')}
            className="test-errors"
          />
        ) : (
          <p className="t-sm t-3">{t('porQueTravou.semLinhas')}</p>
        )}
      </div>
    </section>
  );
}
