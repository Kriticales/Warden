import { createFileRoute } from '@tanstack/react-router';

import { type PacksDialogs, PacksPage } from '../../features/packs/components/PacksPage';

/**
 * Meus packs (T02): a tela inicial do app. A busca abre os diálogos por cima da lista
 * (decisão D38): `criar` abre Criar pack (T03); `pasta` abre Abrir pack (T04) com a pasta
 * escolhida em "Abrir ou importar…".
 */
export const Route = createFileRoute('/packs/')({
  validateSearch: (search: Record<string, unknown>): PacksDialogs => {
    if (typeof search.pasta === 'string' && search.pasta !== '') return { pasta: search.pasta };
    if (search.criar === true || search.criar === 'true') return { criar: true };
    return {};
  },
  component: PacksRoute,
});

function PacksRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <PacksPage
      search={search}
      onSearch={(next) => {
        void navigate({ search: next });
      }}
    />
  );
}
