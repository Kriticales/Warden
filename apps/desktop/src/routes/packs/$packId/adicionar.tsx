import { createFileRoute } from '@tanstack/react-router';

import { AddPanel, PROJECT_KINDS } from '../../../features/add/AddPanel';
import type { ProjectKind } from '../../../lib/ipc/bindings';

/** Busca da rota: `tipo` abre a página já no tipo de onde você clicou (SPEC T08). */
interface AddSearch {
  tipo?: ProjectKind;
}

/**
 * Página Adicionar do pack (SPEC T08; ESTRUTURA N6): em tela cheia, com o menu lateral do pack
 * recolhido para ícones enquanto ela está aberta.
 */
export const Route = createFileRoute('/packs/$packId/adicionar')({
  staticData: { packMenu: 'compact' },
  validateSearch: (search: Record<string, unknown>): AddSearch => {
    const tipo = PROJECT_KINDS.find((kind) => kind === search.tipo);
    return tipo ? { tipo } : {};
  },
  component: AddRoute,
});

function AddRoute() {
  const { packId } = Route.useParams();
  const { tipo } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <AddPanel
      packId={packId}
      kind={tipo ?? 'mod'}
      onKindChange={(kind) => {
        void navigate({ search: kind === 'mod' ? {} : { tipo: kind }, replace: true });
      }}
    />
  );
}
