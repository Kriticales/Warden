import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, type RouterHistory } from '@tanstack/react-router';
import { useState } from 'react';

import { TooltipProvider } from '../components/ui/tooltip';
import { createAppRouter } from './router';

interface AppProps {
  /** Histórico do roteador; os testes usam um em memória. */
  history?: RouterHistory;
  /** Cliente do TanStack Query; os testes passam um novo a cada caso. */
  queryClient?: QueryClient;
}

/** Cliente do TanStack Query do app: erro aparece na hora (sem novas tentativas automáticas). */
export function createQueryClient(): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

/** Raiz da interface: cliente do TanStack Query, tooltips e roteador. */
export function App({ history, queryClient: givenClient }: AppProps) {
  const [queryClient] = useState(() => givenClient ?? createQueryClient());
  const [router] = useState(() => createAppRouter(queryClient, history));
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <RouterProvider router={router} />
      </TooltipProvider>
    </QueryClientProvider>
  );
}
