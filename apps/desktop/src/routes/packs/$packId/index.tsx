import { createFileRoute } from '@tanstack/react-router';

import { PackLanding } from '../../../features/packs/components/PackLanding';

/**
 * Página do pack, provisória (P1-07): o pack recém-criado ou aberto chega aqui. A P1-08
 * substitui este arquivo pelo editor do pack (layout com as 6 seções; abre em Mods).
 */
export const Route = createFileRoute('/packs/$packId/')({
  component: PackRoute,
});

function PackRoute() {
  const { packId } = Route.useParams();
  return <PackLanding packId={packId} />;
}
