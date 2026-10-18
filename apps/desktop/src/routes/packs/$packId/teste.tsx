import { createFileRoute } from '@tanstack/react-router';

import { BisectPage } from '../../../features/bisect/BisectPage';
import { TestPage } from '../../../features/test/TestPage';

/**
 * Busca da tela do teste: `sessao` abre uma sessão gravada (testes anteriores); `modo=busca`, o
 * modo da busca do culpado (D-12).
 */
interface TestSearch {
  sessao?: string;
  modo?: 'busca';
}

/**
 * Tela do teste do pack (SPEC T13). Não é seção do menu: abre pelo botão Testar do cabeçalho,
 * por "Ver último teste" e pelos testes anteriores.
 */
export const Route = createFileRoute('/packs/$packId/teste')({
  validateSearch: (search: Record<string, unknown>): TestSearch => ({
    ...(typeof search.sessao === 'string' && search.sessao !== '' ? { sessao: search.sessao } : {}),
    ...(search.modo === 'busca' ? { modo: 'busca' as const } : {}),
  }),
  component: TestRoute,
});

function TestRoute() {
  const { packId } = Route.useParams();
  const { sessao, modo } = Route.useSearch();
  if (modo === 'busca') {
    return <BisectPage packId={packId} />;
  }
  return <TestPage packId={packId} sessionId={sessao ?? null} />;
}
