import { createFileRoute, redirect } from '@tanstack/react-router';

import { redirectOnFirstRun } from '../features/onboarding/firstRun';

/**
 * A tela inicial do app é Meus packs (SPEC T02). Na primeira execução (T01, P1-13), sem
 * `settings.json`, o app abre nas boas-vindas.
 */
export const Route = createFileRoute('/')({
  beforeLoad: async ({ context }) => {
    await redirectOnFirstRun(context.queryClient);
    // O TanStack Router interrompe a navegação lançando o redirecionamento.
    // eslint-disable-next-line @typescript-eslint/only-throw-error
    throw redirect({ to: '/packs', replace: true });
  },
});
