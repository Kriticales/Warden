/**
 * Chaves do TanStack Query (ARCHITECTURE §18). Dados de um pack ficam sob
 * `['pack', packId, área, …]`, para o evento `pack-changed` invalidar só as áreas que mudaram.
 */
import type { PackArea, PackId } from './bindings';

export const queryKeys = {
  appInfo: ['app', 'info'] as const,
  operations: ['app', 'operations'] as const,
  /** Tudo de um pack. */
  pack: (packId: PackId) => ['pack', packId] as const,
  /** Uma área de um pack; as telas acrescentam o resto da chave depois da área. */
  packArea: (packId: PackId, area: PackArea) => ['pack', packId, area] as const,
};
