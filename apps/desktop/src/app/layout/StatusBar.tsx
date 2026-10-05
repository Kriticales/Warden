/**
 * Rodapé de todas as telas (do app e do pack): o indicador de Tarefas, que abre a gaveta T22,
 * e a versão do Warden. O indicador mostra se há tarefas em andamento (carregador) ou se
 * alguma falhou desde a última vez que a gaveta foi aberta (ícone de erro).
 */
import { CircleAlert, List } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../components/ui/icon';
import { Loader } from '../../components/ui/loader';
import { useAppInfo } from '../../features/about/hooks/useAppInfo';
import { indicatorState, type TasksIndicatorState } from '../tasks/operations';
import { TasksDrawer } from '../tasks/TasksDrawer';
import { useTasksUi } from '../tasks/tasks-store';
import { useOperations } from '../tasks/useOperations';

function IndicatorContent({ state }: { state: TasksIndicatorState }) {
  const { t } = useTranslation('tarefas');
  switch (state.kind) {
    case 'busy':
      return (
        <>
          <Loader />
          <span>{t('indicador.ocupada', { count: state.count })}</span>
        </>
      );
    case 'error':
      return (
        <>
          <Icon icon={CircleAlert} size="sm" />
          <span>{t('indicador.falha', { count: state.count })}</span>
        </>
      );
    case 'idle':
      return (
        <>
          <Icon icon={List} size="sm" />
          <span>{t('indicador.ociosa')}</span>
        </>
      );
  }
}

export function StatusBar() {
  const { t } = useTranslation('navegacao');
  const info = useAppInfo();
  const operations = useOperations();
  const seenUntilMs = useTasksUi((state) => state.seenUntilMs);
  const state = indicatorState(operations.data ?? [], seenUntilMs);
  return (
    <footer className="statusbar">
      <TasksDrawer
        trigger={
          <button
            type="button"
            className={`statusbar__tasks statusbar__tasks--${state.kind}`}
            data-testid="tasks-indicator"
          >
            <IndicatorContent state={state} />
          </button>
        }
      />
      <span className="statusbar__end">
        {info.data
          ? t('rodape.versao', { version: info.data.version })
          : t('rodape.versaoDesconhecida')}
      </span>
    </footer>
  );
}
