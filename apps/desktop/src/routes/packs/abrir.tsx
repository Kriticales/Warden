import { createFileRoute } from '@tanstack/react-router';

import { OpenPackPage } from '../../features/packs/components/OpenPackPage';

interface AbrirSearch {
  /** Pasta escolhida no diálogo nativo (Abrir ou importar…). */
  pasta?: string;
}

/** Abrir pack existente (T04): verifica a pasta escolhida antes de escrever qualquer coisa. */
export const Route = createFileRoute('/packs/abrir')({
  validateSearch: (search: Record<string, unknown>): AbrirSearch =>
    typeof search.pasta === 'string' && search.pasta !== '' ? { pasta: search.pasta } : {},
  component: OpenPackRoute,
});

function OpenPackRoute() {
  const { pasta } = Route.useSearch();
  return <OpenPackPage path={pasta ?? null} />;
}
