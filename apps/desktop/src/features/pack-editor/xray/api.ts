/**
 * O raio-x de um mod (D-10): `xray_mod`. A chave fica embaixo da do relatório do diagnóstico
 * (`diagnosticsKeys.report`): tudo o que invalida o relatório invalida também o raio-x. Isso
 * cobre o que muda o resultado: o inventário (`pack-changed` com a área `inventory`, que é o
 * começo das duas chaves) e o fim de um teste (`invalidateAfterTest`: jars novos na instância,
 * configs da instância e um travamento que pode subir o risco).
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';
import { diagnosticsKeys } from '../../diagnostics/api';

export const xrayKeys = {
  mod: (packId: PackId, path: string) =>
    [...diagnosticsKeys.report(packId), 'raio-x', path] as const,
};

/** "O que este mod altera no jogo". */
export function useXray(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(xrayKeys.mod(packId, path), () => commands.xrayMod(packId, path)),
    enabled,
    staleTime: 30_000,
  });
}
