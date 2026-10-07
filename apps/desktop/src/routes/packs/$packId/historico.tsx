import { createFileRoute } from '@tanstack/react-router';

import { HistoryPage } from '../../../features/versioning/HistoryPage';

/** Seção Histórico do pack (SPEC T17). */
export const Route = createFileRoute('/packs/$packId/historico')({
  component: HistoryRoute,
});

function HistoryRoute() {
  const { packId } = Route.useParams();
  return <HistoryPage packId={packId} />;
}
