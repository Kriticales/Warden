/**
 * Regras puras da lista de resultados (esboço `design/bug-01/adicionar.html`, opção 4; ADR-0068;
 * testadas em `model.test.ts`): o modo de exibição lembrado, o que aparece em cada grupo
 * (bibliotecas escondidas, o que já está no pack) e a ordem pelo nome.
 */
import type { SearchResult } from '../../../lib/ipc/bindings';
import { isInPack } from '../common/model';

/** Lista (tabela densa) ou Cartões (grade). */
export type ViewMode = 'lista' | 'cartoes';

const VIEW_KEY = 'warden.adicionar.modo';

/** O último modo usado (Lista na primeira vez ou se o armazenamento do WebView falhar). */
export function readViewMode(storage: Pick<Storage, 'getItem'> | null = safeStorage()): ViewMode {
  try {
    return storage?.getItem(VIEW_KEY) === 'cartoes' ? 'cartoes' : 'lista';
  } catch {
    return 'lista';
  }
}

export function saveViewMode(
  mode: ViewMode,
  storage: Pick<Storage, 'setItem'> | null = safeStorage(),
): void {
  try {
    storage?.setItem(VIEW_KEY, mode);
  } catch {
    // Sem armazenamento: o modo vale só enquanto a página está aberta.
  }
}

function safeStorage(): Storage | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch {
    return null;
  }
}

/** O que esconder e o que mostrar nos grupos. */
export interface ShowOptions {
  /** Esconder as bibliotecas (padrão sem texto na busca e fora da categoria Bibliotecas). */
  hideLibraries: boolean;
  /** "Mostrar também: O que já está no pack". */
  showInPack: boolean;
  inventoryKeys: ReadonlySet<string>;
}

export interface Shown {
  items: SearchResult[];
  /** As bibliotecas que ficaram de fora (a linha "N bibliotecas escondidas"). */
  hiddenLibraries: SearchResult[];
}

export function shownItems(items: readonly SearchResult[], options: ShowOptions): Shown {
  const shown: SearchResult[] = [];
  const hiddenLibraries: SearchResult[] = [];
  for (const item of items) {
    if (!options.showInPack && isInPack(item, options.inventoryKeys)) continue;
    if (options.hideLibraries && item.isLibrary) hiddenLibraries.push(item);
    else shown.push(item);
  }
  return { items: shown, hiddenLibraries };
}

/** Ordem pelo nome, clicando no título da coluna (só o que já foi carregado). */
export type NameSort = 'asc' | 'desc' | null;

export function nextNameSort(now: NameSort): NameSort {
  return now === null ? 'asc' : now === 'asc' ? 'desc' : null;
}

const collator = new Intl.Collator('pt-BR', { sensitivity: 'base', numeric: true });

export function sortByName(items: readonly SearchResult[], sort: NameSort): SearchResult[] {
  if (sort === null) return [...items];
  const sign = sort === 'asc' ? 1 : -1;
  return [...items].sort((a, b) => sign * collator.compare(a.title, b.title));
}
