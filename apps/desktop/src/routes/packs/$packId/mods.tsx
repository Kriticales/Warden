import { createFileRoute } from '@tanstack/react-router';

import { ModsPage } from '../../../features/pack-editor/mods/ModsPage';

/** Busca da rota: `item` abre os detalhes desse item no painel lateral (SPEC T07). */
interface ModsSearch {
  item?: string;
}

/** Seção Mods (SPEC T06), com os detalhes do item num painel lateral (T07). */
export const Route = createFileRoute('/packs/$packId/mods')({
  validateSearch: (search: Record<string, unknown>): ModsSearch =>
    typeof search.item === 'string' && search.item !== '' ? { item: search.item } : {},
  component: ModsRoute,
});

function ModsRoute() {
  const { packId } = Route.useParams();
  const { item } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <ModsPage
      packId={packId}
      openItem={item ?? null}
      onOpenItem={(path) => {
        void navigate({ search: path ? { item: path } : {}, replace: path === null });
      }}
    />
  );
}
