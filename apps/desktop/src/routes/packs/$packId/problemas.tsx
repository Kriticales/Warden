import { createFileRoute } from '@tanstack/react-router';

import { ProblemsPage } from '../../../features/diagnostics/ProblemsPage';

/** Seção Problemas do pack (SPEC T14; ESTRUTURA §13: "Saúde do pack, problemas e travamentos"). */
export const Route = createFileRoute('/packs/$packId/problemas')({
  component: ProblemsRoute,
});

function ProblemsRoute() {
  const { packId } = Route.useParams();
  return <ProblemsPage packId={packId} />;
}
