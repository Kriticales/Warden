import { createFileRoute, useChildMatches } from '@tanstack/react-router';

import { StandalonePage } from '../features/diagnostics/ai-sem-pack/StandalonePage';

/**
 * Analisar um log (SPEC T14 item 8; ROADMAP D-19): a conversa com o Sculk sem pack, aberta por
 * Meus packs. A conversa aberta é a rota filha (`analisar-log.$conversaId.tsx`), mas a página é
 * uma só: a coluna de conversas fica no lugar ao trocar de conversa.
 */
export const Route = createFileRoute('/analisar-log')({
  component: StandaloneRoute,
});

function StandaloneRoute() {
  const conversaId = useChildMatches({
    select: (matches) => {
      const params: Partial<Record<string, string>> = matches[0]?.params ?? {};
      return params.conversaId ?? null;
    },
  });
  return <StandalonePage conversationId={conversaId} />;
}
