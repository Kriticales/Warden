import { createFileRoute } from '@tanstack/react-router';

import { ModsPage } from '../../../features/pack-editor/mods/ModsPage';

/**
 * Busca da rota: `item` abre os detalhes desse item no painel lateral (SPEC T07); `ver=grafo`
 * mostra a seção no modo Grafo (T06; ARCHITECTURE §18), com `centro` como o mod no centro.
 */
interface ModsSearch {
  item?: string;
  ver?: 'grafo';
  centro?: string;
}

function text(value: unknown): string | undefined {
  return typeof value === 'string' && value !== '' ? value : undefined;
}

/** Seção Mods (SPEC T06), com os detalhes do item num painel lateral (T07). */
export const Route = createFileRoute('/packs/$packId/mods')({
  staticData: { contentWidth: 'wide' },
  validateSearch: (search: Record<string, unknown>): ModsSearch => {
    const item = text(search.item);
    const graph = search.ver === 'grafo';
    const center = graph ? text(search.centro) : undefined;
    return {
      ...(item ? { item } : {}),
      ...(graph ? { ver: 'grafo' as const } : {}),
      ...(center ? { centro: center } : {}),
    };
  },
  component: ModsRoute,
});

function ModsRoute() {
  const { packId } = Route.useParams();
  const { item, ver, centro } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <ModsPage
      packId={packId}
      openItem={item ?? null}
      onOpenItem={(path) => {
        void navigate({
          search: (prev) => {
            const rest = { ...prev };
            delete rest.item;
            return path ? { ...rest, item: path } : rest;
          },
          replace: path === null,
        });
      }}
      view={ver ?? 'lista'}
      center={centro ?? null}
      onView={(view) => {
        void navigate({ search: view === 'grafo' ? { ver: 'grafo' } : {} });
      }}
      onCenter={(path) => {
        void navigate({ search: { ver: 'grafo', centro: path } });
      }}
    />
  );
}
