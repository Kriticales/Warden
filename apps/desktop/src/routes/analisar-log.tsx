import { Outlet, createFileRoute, useChildMatches } from '@tanstack/react-router';

import { StandalonePage } from '../features/diagnostics/ai-sem-pack/StandalonePage';

/**
 * ✦ Analisar um log (SPEC T14 item 8; ROADMAP D-19): a conversa com a IA sem pack, aberta por
 * Meus packs. A conversa aberta é a rota filha (`analisar-log.$conversaId.tsx`).
 */
export const Route = createFileRoute('/analisar-log')({
  component: StandaloneRoute,
});

function StandaloneRoute() {
  const children = useChildMatches();
  if (children.length > 0) {
    return <Outlet />;
  }
  return <StandalonePage />;
}
