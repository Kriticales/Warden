/**
 * Regras puras da lista de mods de um modpack (testadas em `model.test.ts`): o estado de cada mod
 * com o inventário atual (o que acabou de entrar passa a "Já no seu pack" na hora), o filtro "Só
 * os que não estão no seu pack", as contagens e os itens que vão para o `add_plan` conforme o
 * seletor **Versão**.
 */
import type { AddChoice, ModpackMod, ModpackModState } from '../../../lib/ipc/bindings';

/** Seletor **Versão**: a mesma do modpack (testada junto) ou a mais nova compatível. */
export type VersionMode = 'same' | 'newest';

/** As opções do seletor **Versão**, na ordem da tela. */
export const VERSION_MODES: readonly VersionMode[] = ['same', 'newest'];

/** O estado do mod com o inventário atual: o que já entrou no pack conta como "no pack". */
export function modState(mod: ModpackMod, inventoryKeys: ReadonlySet<string>): ModpackModState {
  if (mod.state === 'compatible' && inventoryKeys.has(mod.key)) return 'inPack';
  return mod.state;
}

/** Os mods à vista: todos, ou só os que não estão no pack. */
export function visibleMods(
  mods: readonly ModpackMod[],
  onlyNew: boolean,
  inventoryKeys: ReadonlySet<string>,
): ModpackMod[] {
  return onlyNew ? mods.filter((mod) => modState(mod, inventoryKeys) !== 'inPack') : [...mods];
}

/** Quantos mods em cada estado. */
export function countByState(
  mods: readonly ModpackMod[],
  inventoryKeys: ReadonlySet<string>,
): Record<ModpackModState, number> {
  const counts: Record<ModpackModState, number> = {
    compatible: 0,
    inPack: 0,
    noVersion: 0,
    external: 0,
  };
  for (const mod of mods) counts[modState(mod, inventoryKeys)] += 1;
  return counts;
}

/**
 * Os itens do `add_plan` para os mods escolhidos: com "a mesma do modpack", a versão exata que o
 * modpack usa; com "a mais nova compatível", nenhuma (o plano escolhe).
 */
export function choicesFor(mods: readonly ModpackMod[], mode: VersionMode): AddChoice[] {
  return mods.flatMap((mod) =>
    mod.source !== null && mod.projectId !== null
      ? [
          {
            source: mod.source,
            projectId: mod.projectId,
            versionId: mode === 'same' ? mod.versionId : null,
          },
        ]
      : [],
  );
}
