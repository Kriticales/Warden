import { createFileRoute } from '@tanstack/react-router';

import { TestPage } from '../../../features/test/TestPage';

/** Busca da tela do teste: `sessao` abre uma sessão gravada (testes anteriores). */
interface TestSearch {
  sessao?: string;
}

/**
 * Tela do teste do pack (SPEC T13). Não é seção do menu: abre pelo botão Testar do cabeçalho,
 * por "Ver último teste" e pelos testes anteriores.
 */
export const Route = createFileRoute('/packs/$packId/teste')({
  validateSearch: (search: Record<string, unknown>): TestSearch =>
    typeof search.sessao === 'string' && search.sessao !== '' ? { sessao: search.sessao } : {},
  component: TestRoute,
});

function TestRoute() {
  const { packId } = Route.useParams();
  const { sessao } = Route.useSearch();
  return <TestPage packId={packId} sessionId={sessao ?? null} />;
}
