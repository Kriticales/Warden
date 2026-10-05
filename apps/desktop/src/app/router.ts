import type { QueryClient } from '@tanstack/react-query';
import { createRouter, type RouterHistory } from '@tanstack/react-router';

import { routeTree } from '../routeTree.gen';
import { NotFound } from './layout/NotFound';
import { RouteError } from './layout/RouteError';

/**
 * Cria o roteador (rotas por arquivo em `src/routes/`). Toda rota tem o mesmo *error boundary*
 * (`RouteError`, com "Tentar de novo") e a mesma página de rota inexistente (`NotFound`). Os
 * testes passam um histórico em memória.
 */
export function createAppRouter(queryClient: QueryClient, history?: RouterHistory) {
  return createRouter({
    routeTree,
    context: { queryClient },
    defaultErrorComponent: RouteError,
    defaultNotFoundComponent: NotFound,
    ...(history ? { history } : {}),
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
