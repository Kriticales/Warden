import { createFileRoute, redirect } from '@tanstack/react-router';

/** O pack abre em Mods (SPEC T05). */
export const Route = createFileRoute('/packs/$packId/')({
  beforeLoad: ({ params }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- é assim que o TanStack Router redireciona
    throw redirect({ to: '/packs/$packId/mods', params, replace: true });
  },
});
