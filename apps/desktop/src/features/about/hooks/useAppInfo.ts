/**
 * Versão, commit e plataforma do app (`app_info`). Não muda durante a execução.
 */
import { useQuery } from '@tanstack/react-query';

import { commands } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery } from '../../../lib/ipc/query';

export const appInfoQuery = {
  ...commandQuery(queryKeys.appInfo, commands.appInfo),
  staleTime: Number.POSITIVE_INFINITY,
};

export function useAppInfo() {
  return useQuery(appInfoQuery);
}
