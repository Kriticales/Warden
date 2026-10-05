/**
 * Painel de erro (ARCHITECTURE §5; QUALITY §3): a frase traduzida do `AppError` (o que
 * aconteceu + o que fazer), "Tentar de novo" quando faz sentido e "Detalhes técnicos"
 * recolhidos, com o código `domínio.CÓDIGO`, a tarefa, o `detail` e o botão Copiar.
 *
 * Aceita qualquer erro: `CommandError`, `AppError` cru ou erro inesperado (que vira
 * `app.INTERNAL` com a mensagem original nos detalhes, nunca como frase principal).
 */
import { Copy, RotateCcw } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { appErrorMessage, errorCodeText, technicalDetails, toAppError } from '../../lib/ipc/errors';
import { log } from '../../lib/log';
import { Alert } from '../ui/alert';
import { Button } from '../ui/button';

export interface ErrorPanelProps {
  /** O erro (de uma query, mutation ou operação). */
  error: unknown;
  /** Título acima da frase (por exemplo, "Esta tela não abriu"). Sem ele, a frase é o título. */
  title?: ReactNode;
  /** Mostra "Tentar de novo". */
  onRetry?: () => void;
  /** Outras ações, ao lado de "Tentar de novo". */
  actions?: ReactNode;
  compact?: boolean;
  className?: string | undefined;
}

type CopyState = 'idle' | 'copied' | 'failed';

export function ErrorPanel({
  error,
  title,
  onRetry,
  actions,
  compact = false,
  className,
}: ErrorPanelProps) {
  const { t } = useTranslation();
  const appError = toAppError(error);
  const message = appErrorMessage(appError);
  const [copy, setCopy] = useState<CopyState>('idle');

  const copyDetails = async () => {
    try {
      await navigator.clipboard.writeText(technicalDetails(appError));
      setCopy('copied');
    } catch (cause) {
      log.warn('falha ao copiar os detalhes técnicos', cause);
      setCopy('failed');
    }
  };

  const buttons =
    onRetry || actions ? (
      <>
        {onRetry ? (
          <Button size="sm" icon={RotateCcw} onClick={onRetry}>
            {t('acoes.tentarDeNovo')}
          </Button>
        ) : null}
        {actions}
      </>
    ) : undefined;

  return (
    <Alert
      kind="danger"
      title={title ?? message}
      actions={buttons}
      actionsBelow
      compact={compact}
      className={className}
    >
      {title ? <div className="alert__text">{message}</div> : null}
      <details className="errpanel__details">
        <summary>{t('erro.detalhesTecnicos')}</summary>
        <div className="errpanel__tech">
          <dl className="kv">
            <dt>{t('erro.codigo')}</dt>
            <dd className="t-mono">{errorCodeText(appError.code)}</dd>
            {appError.operationId ? (
              <>
                <dt>{t('erro.operacao')}</dt>
                <dd className="t-mono">{appError.operationId}</dd>
              </>
            ) : null}
          </dl>
          {appError.detail ? <pre className="errpanel__pre">{appError.detail}</pre> : null}
          <div className="row">
            <Button
              size="sm"
              variant="ghost"
              icon={Copy}
              onClick={() => {
                void copyDetails();
              }}
            >
              {t('erro.copiar')}
            </Button>
            <span role="status" className="t-xs t-3">
              {copy === 'copied' ? t('erro.copiado') : null}
              {copy === 'failed' ? t('erro.naoCopiou') : null}
            </span>
          </div>
        </div>
      </details>
    </Alert>
  );
}
