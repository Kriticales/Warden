import type { QueryClient } from '@tanstack/react-query';
import { createRootRouteWithContext, Outlet } from '@tanstack/react-router';

import { SkipLink } from '../app/layout/SkipLink';
import { StatusBar } from '../app/layout/StatusBar';
import { WindowTitleBar } from '../app/layout/TitleBar';
import { useFocusOnNavigation } from '../app/layout/useFocusOnNavigation';
import { useOperationsSync } from '../app/tasks/useOperations';
import { Toaster } from '../components/ui/toast';
import { usePackChangedInvalidation } from '../lib/ipc/events';

export interface RouterContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
});

/**
 * Moldura de todas as telas (ESTRUTURA; HANDOFF §5): "Pular para o conteúdo", a barra de título
 * da janela (só no Windows; UI-01), a página da rota (barra do app ou cabeçalho do pack, e o
 * conteúdo) e o rodapé com Tarefas. Liga também a
 * invalidação por `pack-changed` e a lista de operações por `operation-updated`.
 */
function RootLayout() {
  const { queryClient } = Route.useRouteContext();
  usePackChangedInvalidation(queryClient);
  useOperationsSync();
  useFocusOnNavigation();
  return (
    <>
      <div className="app">
        <SkipLink />
        <WindowTitleBar />
        <Outlet />
        <StatusBar />
      </div>
      <Toaster />
    </>
  );
}
