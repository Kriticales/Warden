/**
 * O **Testar** de cada chave na tela (Chaves e contas): várias podem testar ao mesmo tempo
 * ("Testar todas"), e cada uma lembra o que deu nesta visita. Funcionou ou o serviço recusou
 * fica gravado no backend (`key-checks.json`) e volta em `secrets_overview`; aqui fica só o que
 * não vira situação da chave: "esta versão não sabe testar" e "não deu para testar agora".
 */
import { useCallback, useState } from 'react';

import type { AppError, SecretKind } from '../../../lib/ipc/bindings';
import { toAppError } from '../../../lib/ipc/errors';
import { useTestSecret } from '../api';

export type KeyTest =
  | { status: 'testing' }
  | { status: 'done' }
  | { status: 'notTestable' }
  /** Não foi o serviço recusando (internet, tempo esgotado…): a situação gravada não mudou. */
  | { status: 'failed'; error: AppError };

/** Os erros que querem dizer "o serviço recusou a chave" (os mesmos do Rust, `key_checks`). */
export function isRefusal(error: AppError): boolean {
  const { domain, code } = error.code;
  return (
    ((domain === 'ai' || domain === 'curseforge') && code === 'KEY_INVALID') ||
    (domain === 'versioning' && code === 'GITHUB_TOKEN_REJECTED')
  );
}

export function useKeyTests() {
  const [tests, setTests] = useState<Partial<Record<SecretKind, KeyTest>>>({});
  const mutation = useTestSecret();
  const { mutateAsync } = mutation;

  const run = useCallback(
    async (kind: SecretKind) => {
      setTests((current) => ({ ...current, [kind]: { status: 'testing' } }));
      let next: KeyTest;
      try {
        const result = await mutateAsync(kind);
        next = result.status === 'notTestable' ? { status: 'notTestable' } : { status: 'done' };
      } catch (error) {
        const appError = toAppError(error);
        next = isRefusal(appError) ? { status: 'done' } : { status: 'failed', error: appError };
      }
      setTests((current) => ({ ...current, [kind]: next }));
    },
    [mutateAsync],
  );

  /** Esquece o que o teste desta visita disse (a chave foi trocada ou removida). */
  const forget = useCallback((kind: SecretKind) => {
    setTests((current) =>
      Object.fromEntries(Object.entries(current).filter(([entry]) => entry !== kind)),
    );
  }, []);

  return { tests, run, forget };
}
