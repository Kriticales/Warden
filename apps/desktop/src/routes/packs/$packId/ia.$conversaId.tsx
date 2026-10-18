import { createFileRoute } from '@tanstack/react-router';

import { ConversationPage } from '../../../features/diagnostics/ai/ConversationPage';

/** Uma conversa com a IA (SPEC T14 ✦ passo 2), com "← Conversas". */
export const Route = createFileRoute('/packs/$packId/ia/$conversaId')({
  component: ConversationRoute,
});

function ConversationRoute() {
  const { packId, conversaId } = Route.useParams();
  return <ConversationPage key={conversaId} packId={packId} conversationId={conversaId} />;
}
