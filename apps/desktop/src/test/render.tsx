/**
 * Renderização nos testes: o app inteiro numa rota (`renderApp`) ou um componente com os
 * provedores que ele espera (`renderWithProviders`: TanStack Query, tooltips e toasts).
 */
import { QueryClientProvider } from '@tanstack/react-query';
import { createMemoryHistory } from '@tanstack/react-router';
import { render, type RenderResult } from '@testing-library/react';
import type { ReactNode } from 'react';

import { App, createQueryClient } from '../app/App';
import { Toaster } from '../components/ui/toast';
import { TooltipProvider } from '../components/ui/tooltip';

export function renderApp(path = '/'): RenderResult {
  return render(
    <App
      history={createMemoryHistory({ initialEntries: [path] })}
      queryClient={createQueryClient()}
    />,
  );
}

export function renderWithProviders(ui: ReactNode): RenderResult {
  const queryClient = createQueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        {ui}
        <Toaster />
      </TooltipProvider>
    </QueryClientProvider>,
  );
}
