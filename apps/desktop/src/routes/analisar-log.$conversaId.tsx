import { createFileRoute } from '@tanstack/react-router';

/**
 * Uma conversa sem pack (SPEC T14 item 8). Quem desenha é a página (`analisar-log.tsx`), que lê o
 * `conversaId` desta rota.
 */
export const Route = createFileRoute('/analisar-log/$conversaId')({
  component: () => null,
});
