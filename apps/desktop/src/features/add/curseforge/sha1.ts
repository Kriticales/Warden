/**
 * Conferência do arquivo entre as fontes (SPEC T08): a versão escolhida numa fonte tem o mesmo
 * SHA-1 de alguma versão da outra? Se sim, é o mesmo arquivo; se não, a tela avisa.
 */
import type { VersionOption } from '../../../lib/ipc/bindings';

export type Sha1Match = 'same' | 'different' | 'unknown';

/**
 * - `unknown`: a versão escolhida não informou o SHA-1 (nada a comparar).
 * - `same`: alguma versão da outra fonte tem o mesmo SHA-1.
 * - `different`: nenhuma tem (inclusive quando a outra fonte não tem versão para o pack).
 */
export function compareSha1(
  chosen: string | null,
  others: readonly Pick<VersionOption, 'sha1'>[],
): Sha1Match {
  const wanted = chosen?.trim().toLowerCase();
  if (!wanted) return 'unknown';
  return others.some((version) => version.sha1?.trim().toLowerCase() === wanted)
    ? 'same'
    : 'different';
}
