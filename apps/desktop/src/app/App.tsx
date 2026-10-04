import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, type RouterHistory } from '@tanstack/react-router';
import { useState } from 'react';

import { createAppRouter } from './router';

interface AppProps {
  /** Histórico do roteador; os testes usam um em memória. */
  history?: RouterHistory;
}

/** Raiz da interface: cliente do TanStack Query e roteador. */
export function App({ history }: AppProps) {
  const [queryClient] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  const [router] = useState(() => createAppRouter(queryClient, history));
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
