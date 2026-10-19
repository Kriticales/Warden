import { createFileRoute } from '@tanstack/react-router';

import { StandaloneConversation } from '../features/diagnostics/ai-sem-pack/StandaloneConversation';

/** Uma conversa sem pack (SPEC T14 item 8), com "← Analisar um log". */
export const Route = createFileRoute('/analisar-log/$conversaId')({
  component: StandaloneConversationRoute,
});

function StandaloneConversationRoute() {
  const { conversaId } = Route.useParams();
  return <StandaloneConversation key={conversaId} conversationId={conversaId} />;
}
