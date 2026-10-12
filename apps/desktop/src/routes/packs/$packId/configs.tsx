import { createFileRoute } from '@tanstack/react-router';

import { ConfigsPage, type ConfigsSearch } from '../../../features/configs/ConfigsPage';

/** Seção Configs do pack (SPEC T12): o arquivo e a origem abertos ficam na URL. */
export const Route = createFileRoute('/packs/$packId/configs')({
  validateSearch: (search: Record<string, unknown>): ConfigsSearch => ({
    ...(search.origem === 'instancia' ? { origem: 'instancia' as const } : {}),
    ...(typeof search.arquivo === 'string' && search.arquivo !== ''
      ? { arquivo: search.arquivo }
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
