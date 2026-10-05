/**
 * Gaveta de Tarefas (T22): operações em andamento com progresso e Cancelar (quando a operação
 * permite) e as últimas 50 concluídas com o resultado; erro com "Ver detalhes". Abre pelo
 * indicador do rodapé, em todas as telas do app e do pack.
 */
import { CircleCheck, CircleMinus, CircleX, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { ProgressBar } from '../../components/common/ProgressBar';
import { Button } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Sheet, SheetContent, SheetTrigger } from '../../components/ui/sheet';
import { commands, type OperationSnapshot } from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { useCommandMutation } from '../../lib/ipc/query';
import {
  finishedWhen,
  operationName,
  progressPercent,
  progressText,
  stageLabel,
} from './operation-text';
import { isFinished } from './operations';
import { useTasksUi } from './tasks-store';
import { useOperations } from './useOperations';

function RunningTask({ operation }: { operation: OperationSnapshot }) {
  const { t } = useTranslation('tarefas');
  const cancel = useCommandMutation(commands.operationCancel, {
    invalidates: [queryKeys.operations],
  });
  const name = operationName(operation.kind);
  const waiting = operation.state === 'waitingForLock';
  const cancelling = operation.state === 'cancelling' || cancel.isPending;
  const meta = [
    waiting ? t('aguardandoTrava') : stageLabel(operation),
    operation.progress ? progressText(operation.progress) : null,
  ].filter((part): part is string => Boolean(part));

  return (
    <li className="panel task">
      <ProgressBar
        size="sm"
        label={name}
        value={waiting ? null : progressPercent(operation.progress)}
        state={waiting ? 'waiting' : 'running'}
        {...(meta.length > 0 ? { meta: meta.join(' · ') } : {})}
      />
      {operation.cancellable || cancelling ? (
        <div className="task__actions">
          <Button
            size="sm"
            variant="ghost"
            loading={cancelling}
            aria-label={cancelling ? undefined : t('cancelarNome', { nome: name })}
            onClick={() => {
              cancel.mutate(operation.id);
            }}
          >
            {cancelling ? t('cancelando') : t('cancelar')}
          </Button>
        </div>
      ) : null}
      {cancel.error ? <ErrorPanel error={cancel.error} compact /> : null}
    </li>
  );
}

const RESULT: Record<
  'succeeded' | 'failed' | 'cancelled',
  {
    icon: LucideIcon;
    className: string;
    key: 'resultado.ok' | 'resultado.falhou' | 'resultado.cancelada';
  }
> = {
  succeeded: { icon: CircleCheck, className: 'task__done--ok', key: 'resultado.ok' },
  failed: { icon: CircleX, className: 'task__done--failed', key: 'resultado.falhou' },
  cancelled: { icon: CircleMinus, className: 'task__done--cancelled', key: 'resultado.cancelada' },
};

function FinishedTask({ operation }: { operation: OperationSnapshot }) {
  const { t } = useTranslation('tarefas');
  const expanded = useTasksUi((state) => state.expandedId === operation.id);
  const toggleDetails = useTasksUi((state) => state.toggleDetails);
  if (
    operation.state !== 'succeeded' &&
    operation.state !== 'failed' &&
    operation.state !== 'cancelled'
  ) {
    return null;
  }
  const result = RESULT[operation.state];
  const failed = operation.state === 'failed';
  return (
    <li className={`task__done ${result.className}`}>
      <Icon icon={result.icon} />
      <span>
        {t('linhaComResultado', {
          nome: operationName(operation.kind),
          resultado: t(result.key),
          quando: finishedWhen(operation.finishedAtMs ?? operation.startedAtMs),
        })}
        {failed ? (
          <>
            {' '}
            <Button
              variant="link"
              aria-expanded={expanded}
              onClick={() => {
                toggleDetails(operation.id);
              }}
            >
              {expanded ? t('ocultarDetalhes') : t('verDetalhes')}
            </Button>
          </>
        ) : null}
      </span>
      {failed && expanded ? (
        <div className="task__done-error">
          <ErrorPanel error={operation.error} compact />
        </div>
      ) : null}
    </li>
  );
}

function TasksBody() {
  const { t } = useTranslation('tarefas');
  const operations = useOperations();

  if (operations.isPending) {
    return <LoadingState label={t('carregando')} />;
  }
  if (operations.isError) {
    return (
      <ErrorPanel
        error={operations.error}
        onRetry={() => {
          void operations.refetch();
        }}
      />
    );
  }
  const running = operations.data.filter((operation) => !isFinished(operation));
  const finished = operations.data.filter(isFinished);
  if (running.length === 0 && finished.length === 0) {
    return (
      <EmptyState
        compact
        glyph="clock"
        title={t('vazio.titulo')}
        text={t('vazio.texto')}
        headingLevel={3}
      />
    );
  }
  return (
    <>
      {running.length > 0 ? (
        <section className="tasks__group" aria-labelledby="tarefas-andamento">
          <h3 className="t-caps t-3" id="tarefas-andamento">
            {t('emAndamento')}
          </h3>
          <ul className="tasks__list">
            {running.map((operation) => (
              <RunningTask key={operation.id} operation={operation} />
            ))}
          </ul>
        </section>
      ) : null}
      {finished.length > 0 ? (
        <section className="tasks__group" aria-labelledby="tarefas-concluidas">
          <h3 className="t-caps t-3" id="tarefas-concluidas">
            {t('concluidas')}
          </h3>
          <ul className="checklist">
            {finished.map((operation) => (
              <FinishedTask key={operation.id} operation={operation} />
            ))}
          </ul>
        </section>
      ) : null}
      <p className="t-xs t-3">{t('limite')}</p>
    </>
  );
}

/**
 * A gaveta, com o elemento que a abre (`trigger`, o indicador do rodapé): o Radix liga
 * `aria-haspopup`/`aria-expanded` nele e devolve o foco a ele ao fechar.
 */
export function TasksDrawer({ trigger }: { trigger: ReactNode }) {
  const { t } = useTranslation('tarefas');
  const open = useTasksUi((state) => state.open);
  const setOpen = useTasksUi((state) => state.setOpen);
  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>{trigger}</SheetTrigger>
      <SheetContent title={t('titulo')}>
        <TasksBody />
      </SheetContent>
    </Sheet>
  );
}
