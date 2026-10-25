import { createFileRoute, redirect } from '@tanstack/react-router';

import { DEFAULT_SETTINGS_PAGE } from '../features/settings/pages';

/** `/configuracoes` abre a primeira página (Geral). */
export const Route = createFileRoute('/configuracoes/')({
  beforeLoad: () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- é assim que o TanStack Router redireciona
    throw redirect({
      to: '/configuracoes/$pagina',
      params: { pagina: DEFAULT_SETTINGS_PAGE },
      replace: true,
    });
  },
});
