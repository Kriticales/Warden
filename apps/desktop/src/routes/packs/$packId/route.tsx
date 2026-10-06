import { createFileRoute } from '@tanstack/react-router';

import { PackLayout } from '../../../features/pack-editor/layout/PackLayout';

/**
 * Pack aberto (SPEC T05): cabeçalho fixo e menu lateral com as 6 seções; cada seção é uma rota
 * filha (`mods.tsx`; as outras chegam com as tarefas donas, registradas em
 * `features/pack-editor/sections.ts`).
 */
export const Route = createFileRoute('/packs/$packId')({
  component: PackRoute,
});

function PackRoute() {
  const { packId } = Route.useParams();
  return <PackLayout packId={packId} />;
}
