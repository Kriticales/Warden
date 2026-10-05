/**
 * Primeira execução (T01): o assistente só aparece se não existir `settings.json`. O app abre
 * em Meus packs (`/`); lá, `redirectOnFirstRun` manda para `/boas-vindas` quando o
 * `settings_status` diz que é a primeira vez.
 *
 * Se o estado não puder ser lido, o app segue para a tela pedida: não prender o usuário fora
 * do app por causa de uma leitura que falhou (o erro vai para os registros).
 */
import type { QueryClient } from '@tanstack/react-query';
import { redirect } from '@tanstack/react-router';

import { log } from '../../lib/log';
import { settingsStatusQuery } from '../settings/api';

export async function redirectOnFirstRun(queryClient: QueryClient): Promise<void> {
  let firstRun = false;
  try {
    firstRun = (await queryClient.query(settingsStatusQuery)).firstRun;
  } catch (error) {
    log.warn('não foi possível saber se é a primeira execução', error);
  }
  if (firstRun) {
    // O TanStack Router interrompe a navegação lançando o redirecionamento.
    // eslint-disable-next-line @typescript-eslint/only-throw-error
    throw redirect({ to: '/boas-vindas', replace: true });
  }
}
