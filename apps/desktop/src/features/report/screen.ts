/**
 * Onde a pessoa está, nos termos da interface ("Pack › Mods"), e o pack aberto: o relato
 * (UI-02) leva os dois. Vêm da rota mais funda que está aberta.
 */
import type { TFunction } from 'i18next';

import type { PackId } from '../../lib/ipc/bindings';

type ScreenKey =
  | 'desconhecida'
  | 'primeiraExecucao'
  | 'configuracoes'
  | 'meusPacks'
  | 'criarPack'
  | 'abrirPack'
  | 'pack'
  | 'mods'
  | 'adicionar'
  | 'historico'
  | 'exportar'
  | 'testar';

/** Tela de cada rota (o `routeId` do TanStack Router). */
const SCREENS: Readonly<Record<string, ScreenKey>> = {
  '/boas-vindas': 'primeiraExecucao',
  '/configuracoes': 'configuracoes',
  '/': 'meusPacks',
  '/packs/': 'meusPacks',
  '/packs/novo': 'criarPack',
  '/packs/abrir': 'abrirPack',
  '/packs/$packId': 'pack',
  '/packs/$packId/': 'pack',
  '/packs/$packId/mods': 'mods',
  '/packs/$packId/adicionar': 'adicionar',
  '/packs/$packId/historico': 'historico',
  '/packs/$packId/exportar': 'exportar',
  '/packs/$packId/teste': 'testar',
};

/** Uma rota aberta: o id e os parâmetros. */
export interface OpenRoute {
  routeId: string;
  params: Record<string, string | undefined>;
}

/** A tela e o pack da rota mais funda. */
export function screenOf(
  matches: readonly OpenRoute[],
  t: TFunction<'relato'>,
): { screen: string; packId: PackId | null } {
  const deepest = matches.at(-1);
  const key = (deepest && SCREENS[deepest.routeId]) ?? 'desconhecida';
  const packId = deepest?.params.packId ?? null;
  return { screen: t(`telas.${key}`), packId };
}
