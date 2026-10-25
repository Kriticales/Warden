import { createFileRoute } from '@tanstack/react-router';

import { ConfigsPage, type ConfigsSearch } from '../../../features/configs/ConfigsPage';

/** Seção Configs do pack (SPEC T12; ADR-0067): o arquivo e a vista abertos ficam na URL. */
export const Route = createFileRoute('/packs/$packId/configs')({
  // Três colunas na largura toda, com o menu do pack só em ícones (esboço aprovado
  // `design/bug-01/configs.html`).
  staticData: { packMenu: 'compact', contentWidth: 'full' },
  validateSearch: (search: Record<string, unknown>): ConfigsSearch => ({
    ...(search.origem === 'instancia' || search.origem === 'pack' || search.origem === 'comparar'
      ? { origem: search.origem }
      : {}),
    ...(typeof search.arquivo === 'string' && search.arquivo !== ''
      ? { arquivo: search.arquivo }
      : {}),
    ...(typeof search.linha === 'number' && Number.isInteger(search.linha) && search.linha > 0
      ? { linha: search.linha }
      : {}),
  }),
  component: ConfigsRoute,
});

function ConfigsRoute() {
  const { packId } = Route.useParams();
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <ConfigsPage
      packId={packId}
      search={search}
      onNavigate={(next) => {
        void navigate({ search: next });
      }}
    />
  );
}
