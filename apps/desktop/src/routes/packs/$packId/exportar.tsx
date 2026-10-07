import { createFileRoute } from '@tanstack/react-router';

import { ExportPage } from '../../../features/export/ExportPage';

/** Seção Exportar do pack (SPEC T19). */
export const Route = createFileRoute('/packs/$packId/exportar')({
  component: ExportRoute,
});

function ExportRoute() {
  const { packId } = Route.useParams();
  return <ExportPage packId={packId} />;
}
