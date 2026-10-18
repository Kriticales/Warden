import { Outlet, createFileRoute, useChildMatches } from '@tanstack/react-router';

import { AiPage } from '../../../features/diagnostics/ai/AiPage';

/** Busca da seção: `sessao` começa uma conversa sobre essa sessão do Testar. */
interface AiSearch {
  sessao?: string;
}

/**
 * Seção ✦ Diagnóstico com IA do pack (SPEC T14; ESTRUTURA §13: "Conversar com a IA sobre um
 * problema do pack"). A conversa aberta é a rota filha (`ia.$conversaId.tsx`).
 */
export const Route = createFileRoute('/packs/$packId/ia')({
  validateSearch: (search: Record<string, unknown>): AiSearch =>
    typeof search.sessao === 'string' && search.sessao !== '' ? { sessao: search.sessao } : {},
  component: AiRoute,
});

function AiRoute() {
  const { packId } = Route.useParams();
  const { sessao } = Route.useSearch();
  const children = useChildMatches();
  if (children.length > 0) {
    return <Outlet />;
  }
  return <AiPage key={sessao ?? ''} packId={packId} sessionId={sessao ?? null} />;
}
