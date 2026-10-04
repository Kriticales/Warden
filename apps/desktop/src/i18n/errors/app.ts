import type { AppErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros do domínio `app`: o que aconteceu + o que fazer (QUALITY §3). */
export const app: Record<AppErrorCode, string> = {
  INTERNAL:
    'Algo deu errado dentro do Warden. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
