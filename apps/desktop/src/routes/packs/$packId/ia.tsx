import { createFileRoute, useChildMatches } from '@tanstack/react-router';

import { SculkPage } from '../../../features/diagnostics/ai/SculkPage';

/** Busca da seção: `sessao` começa uma conversa sobre essa sessão do Testar. */
interface AiSearch {
  sessao?: string;
}

/**
 * Seção Sensor Sculk do pack (SPEC T14; ESTRUTURA §13: "Conversar com o Sculk sobre um problema
 * do pack"; protótipo `design/bug-01/sensor-sculk.html`). A conversa aberta é a rota filha
 * (`ia.$conversaId.tsx`), mas a página é uma só: a coluna de conversas fica no lugar ao trocar
 * de conversa. A seção usa a área inteira e o menu do pack fica só com os ícones.
 */
export const Route = createFileRoute('/packs/$packId/ia')({
  validateSearch: (search: Record<string, unknown>): AiSearch =>
    typeof search.sessao === 'string' && search.sessao !== '' ? { sessao: search.sessao } : {},
  staticData: { packMenu: 'compact' },
  component: AiRoute,
});

function AiRoute() {
  const { packId } = Route.useParams();
  const { sessao } = Route.useSearch();
  const conversaId = useChildMatches({
    select: (matches) => {
      const params: Partial<Record<string, string>> = matches[0]?.params ?? {};
      return params.conversaId ?? null;
    },
  });
  return (
    <SculkPage
      packId={packId}
      conversationId={conversaId}
      sessionId={conversaId ? null : (sessao ?? null)}
    />
  );
}
