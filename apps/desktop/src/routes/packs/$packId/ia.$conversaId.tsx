import { createFileRoute } from '@tanstack/react-router';

/**
 * Uma conversa com o Sculk (SPEC T14 passo 2). Quem desenha é a seção (`ia.tsx`), que lê o
 * `conversaId` desta rota: a coluna de conversas fica no lugar ao trocar de conversa.
 */
export const Route = createFileRoute('/packs/$packId/ia/$conversaId')({
  component: () => null,
});
