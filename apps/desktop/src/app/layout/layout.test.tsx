import { QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRouteWithContext,
  createRoute,
  createRouter,
  Link,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { TooltipProvider } from '../../components/ui/tooltip';
import { axePage } from '../../test/axe';
import { mockBackend } from '../../test/backend';
import { createQueryClient } from '../App';
import { AppPage } from './AppPage';
import { focusMain, focusPageTitle } from './focus';
import { PageHead } from './PageHead';
import { RouteError } from './RouteError';
import { useFocusOnNavigation } from './useFocusOnNavigation';

/**
 * Roteador de teste com a moldura real (AppPage, RouteError, foco na troca de tela) e rotas
 * próprias: "/" com um link, "/b" com título, "/quebra" que lança e "/pack/filha" que lança
 * dentro de um layout.
 */
function renderRouter(path: string, failures: { broken: boolean }) {
  const queryClient = createQueryClient();
  const root = createRootRouteWithContext<{ queryClient: typeof queryClient }>()({
    component: function Root() {
      useFocusOnNavigation();
      return (
        <div className="app">
          <Outlet />
        </div>
      );
    },
  });
  const home = createRoute({
    getParentRoute: () => root,
    path: '/',
    component: () => (
      <AppPage where="Início">
        <PageHead title="Início" sub="Página de teste" />
        {/* Rota só deste teste: o tipo do roteador do app não a conhece. */}
        <Link to={'/b' as '/'}>Ir para B</Link>
      </AppPage>
    ),
  });
  const b = createRoute({
    getParentRoute: () => root,
    path: '/b',
    component: () => (
      <AppPage back={{ to: '/', label: 'Meus packs' }}>
        <PageHead title="Página B" />
      </AppPage>
    ),
  });
  const broken = createRoute({
    getParentRoute: () => root,
    path: '/quebra',
    component: function Broken() {
      if (failures.broken) {
        throw new Error('falha de propósito');
      }
      return (
        <AppPage>
          <PageHead title="Consertou" />
        </AppPage>
      );
    },
  });
  const pack = createRoute({
    getParentRoute: () => root,
    path: '/pack',
    component: () => (
      <AppPage>
        <PageHead title="Pack" />
        <Outlet />
      </AppPage>
    ),
  });
  const child = createRoute({
    getParentRoute: () => pack,
    path: '/filha',
    component: function Child() {
      throw new Error('filha quebrou');
    },
  });
  const router = createRouter({
    routeTree: root.addChildren([home, b, broken, pack.addChildren([child])]),
    context: { queryClient },
    defaultErrorComponent: RouteError,
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <RouterProvider router={router} />
      </TooltipProvider>
    </QueryClientProvider>,
  );
}

describe('moldura das telas', () => {
  it('barra do app com "← Voltar", marca e onde você está; passa no axe', async () => {
    mockBackend();
    const { container } = renderRouter('/b', { broken: false });
    expect(await screen.findByRole('link', { name: 'Meus packs' })).toBeDefined();
    expect(screen.getByRole('banner').textContent).toContain('Warden');
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('a primeira tela não rouba o foco; a troca de tela leva o foco ao título', async () => {
    mockBackend();
    const user = userEvent.setup();
    renderRouter('/', { broken: false });
    const link = await screen.findByRole('link', { name: 'Ir para B' });
    expect(screen.getByText('Início', { selector: '.topbar__where' })).toBeDefined();
    expect(document.activeElement).toBe(document.body);
    await user.click(link);
    const title = await screen.findByRole('heading', { level: 1, name: 'Página B' });
    await waitFor(() => {
      expect(document.activeElement).toBe(title);
    });
  });

  it('erro numa página do app: tela inteira com a barra, frase e "Tentar de novo" que conserta', async () => {
    const backend = mockBackend();
    const user = userEvent.setup();
    const failures = { broken: true };
    renderRouter('/quebra', failures);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Esta tela não abriu' }),
    ).toBeDefined();
    expect(screen.getByRole('banner')).toBeDefined();
    expect(screen.getByRole('main')).toBeDefined();
    expect(screen.getByText(/falha de propósito/).closest('pre')).not.toBeNull();
    await waitFor(() => {
      expect(
        backend
          .callsOf('plugin:log|log')
          .some((call) => String(call.args.message).includes('/quebra')),
      ).toBe(true);
    });
    failures.broken = false;
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Consertou' })).toBeDefined();
  });

  it('erro dentro de um layout: só o conteúdo, sem uma segunda barra', async () => {
    mockBackend();
    renderRouter('/pack/filha', { broken: false });
    expect(await screen.findByRole('heading', { name: 'Esta tela não abriu' })).toBeDefined();
    expect(screen.getAllByRole('banner')).toHaveLength(1);
    expect(screen.getAllByRole('main')).toHaveLength(1);
  });
});

describe('foco por script', () => {
  it('sem título vai para o <main>; sem <main> não faz nada', () => {
    document.body.innerHTML = '<main id="conteudo" tabindex="-1"></main>';
    expect(focusPageTitle()).toBe(true);
    expect(document.activeElement?.id).toBe('conteudo');
    document.body.innerHTML = '';
    expect(focusMain()).toBe(false);
    expect(focusPageTitle()).toBe(false);
  });
});
