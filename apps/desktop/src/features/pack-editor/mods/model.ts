/**
 * Lógica da lista de Mods (SPEC T06), sem React: filtros, ordem, agrupamento e as linhas da
 * tabela virtualizada.
 *
 * O agrupamento é genérico (`groupItems` recebe a função que dá o grupo de cada item): a lista
 * agrupa por tipo; a W-08 (Warden 1.1) acrescenta "Agrupar por: grupo" com outra função, sem
 * mexer na tabela (gancho 1.1, ADR-0039).
 */
import type { InventoryItem, ItemKind, ItemSide, ItemSource } from '../../../lib/ipc/bindings';

/** Filtros da barra de ferramentas. `null` = todos. */
export interface ModsFilters {
  search: string;
  kind: ItemKind | null;
  source: ItemSource | null;
  side: ItemSide | null;
  show: 'all' | 'problems' | 'pinned' | 'optional';
}

export const NO_FILTERS: ModsFilters = {
  search: '',
  kind: null,
  source: null,
  side: null,
  show: 'all',
};

export function hasFilters(filters: ModsFilters): boolean {
  return (
    filters.search.trim() !== '' ||
    filters.kind !== null ||
    filters.source !== null ||
    filters.side !== null ||
    filters.show !== 'all'
  );
}

/** Item com problema que a lista sabe ver sozinha: arquivo inválido ou fora do índice. */
export function hasProblem(item: InventoryItem): boolean {
  return item.state !== 'ok';
}

/** Quantos itens estão no pack (no índice, inclusive os inválidos; sem os de fora). */
export function itemsInPack(items: readonly InventoryItem[]): number {
  return items.filter((item) => item.state !== 'outsideIndex').length;
}

/** Busca sem diferenciar maiúsculas nem acentos, no nome e no arquivo. */
export function normalize(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

export function matches(item: InventoryItem, filters: ModsFilters): boolean {
  const search = normalize(filters.search.trim());
  if (
    search !== '' &&
    !normalize(item.name).includes(search) &&
    !normalize(item.fileName ?? '').includes(search)
  ) {
    return false;
  }
  if (filters.kind !== null && item.kind !== filters.kind) return false;
  if (filters.source !== null && item.source !== filters.source) return false;
  if (filters.side !== null && item.side !== filters.side) return false;
  switch (filters.show) {
    case 'all':
      return true;
    case 'problems':
      return hasProblem(item);
    case 'pinned':
      return item.pinned;
    case 'optional':
      return item.optional;
  }
}

const collator = new Intl.Collator('pt-BR', { sensitivity: 'base', numeric: true });

/** Ordem por nome (A→Z ou Z→A). */
export function sortItems(
  items: readonly InventoryItem[],
  direction: 'asc' | 'desc',
): InventoryItem[] {
  const sorted = [...items].sort((a, b) => collator.compare(a.name, b.name));
  return direction === 'asc' ? sorted : sorted.reverse();
}

/** Um grupo da lista. */
export interface ItemGroup<K extends string = string> {
  key: K;
  items: InventoryItem[];
}

/**
 * Agrupa preservando a ordem dos itens. `order` dá a ordem dos grupos (os que não estão nela
 * vêm depois, na ordem em que aparecem); grupos vazios não aparecem.
 */
export function groupItems<K extends string>(
  items: readonly InventoryItem[],
  groupOf: (item: InventoryItem) => K,
  order: readonly K[] = [],
): ItemGroup<K>[] {
  const groups = new Map<K, InventoryItem[]>();
  for (const key of order) {
    groups.set(key, []);
  }
  for (const item of items) {
    const key = groupOf(item);
    const list = groups.get(key);
    if (list) {
      list.push(item);
    } else {
      groups.set(key, [item]);
    }
  }
  return [...groups.entries()]
    .filter(([, list]) => list.length > 0)
    .map(([key, list]) => ({ key, items: list }));
}

/** Ordem dos grupos por tipo (SPEC T06: Mods, Resource packs, Shaders). */
export const KIND_ORDER: readonly ItemKind[] = ['mod', 'resourcePack', 'shader', 'other'];

export function groupByKind(items: readonly InventoryItem[]): ItemGroup<ItemKind>[] {
  return groupItems(items, (item) => item.kind, KIND_ORDER);
}

/** Linha da tabela virtualizada: o cabeçalho de um grupo ou um item. */
export type TableRow<K extends string = string> =
  | { type: 'group'; key: K; count: number; collapsed: boolean }
  | { type: 'item'; group: K; item: InventoryItem };

/** As linhas da tabela, com os grupos recolhidos sem os itens. */
export function tableRows<K extends string>(
  groups: readonly ItemGroup<K>[],
  collapsed: ReadonlySet<K>,
): TableRow<K>[] {
  const rows: TableRow<K>[] = [];
  for (const group of groups) {
    const isCollapsed = collapsed.has(group.key);
    rows.push({ type: 'group', key: group.key, count: group.items.length, collapsed: isCollapsed });
    if (!isCollapsed) {
      for (const item of group.items) {
        rows.push({ type: 'item', group: group.key, item });
      }
    }
  }
  return rows;
}

/** Contagem por tipo dos itens no pack (o resumo do cabeçalho da seção). */
export function countByKind(items: readonly InventoryItem[]): Record<ItemKind, number> {
  const counts: Record<ItemKind, number> = { mod: 0, resourcePack: 0, shader: 0, other: 0 };
  for (const item of items) {
    if (item.state !== 'outsideIndex') {
      counts[item.kind] += 1;
    }
  }
  return counts;
}

/** Os itens selecionados que ainda existem na lista (a lista pode ter mudado no disco). */
export function selectedItems(
  items: readonly InventoryItem[],
  selected: ReadonlySet<string>,
): InventoryItem[] {
  return items.filter((item) => selected.has(item.path));
}
