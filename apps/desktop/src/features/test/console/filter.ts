/**
 * Filtros do console do jogo (SPEC T13): nível ("Todas as linhas", "Só avisos e erros", "Só
 * erros") e busca. Funções puras: o texto mostrado de cada linha vem de quem chama, para que
 * as linhas do Warden sejam buscadas pela frase traduzida, não pela chave do catálogo.
 */
import type { ConsoleLevel, ConsoleLine } from '../../../lib/ipc/bindings';

/** O seletor de nível da barra do console. */
export type LevelFilter = 'all' | 'warn' | 'error';

/** A cor da linha: as classes `.console__line--<tom>` do design system. */
export type LineTone = 'info' | 'warn' | 'error' | 'debug' | 'warden';

/** Uma linha que passou no filtro, com o nível que vale para ela. */
export interface ConsoleRow {
  line: ConsoleLine;
  /**
   * O nível da linha ou, se ela não tem nível (continuação de um stack trace, por exemplo), o
   * da última linha com nível antes dela. `null` antes da primeira linha com nível.
   */
  level: ConsoleLevel | null;
}

export interface ConsoleFilter {
  level: LevelFilter;
  query: string;
}

const RANK: Record<ConsoleLevel, number> = {
  trace: 0,
  debug: 1,
  info: 2,
  warn: 3,
  error: 4,
  fatal: 5,
};

const MIN_RANK: Record<LevelFilter, number> = { all: -1, warn: RANK.warn, error: RANK.error };

/** A linha entra no seletor de nível? Sem nível conhecido, só em "Todas as linhas". */
export function matchesLevel(level: ConsoleLevel | null, filter: LevelFilter): boolean {
  if (filter === 'all') {
    return true;
  }
  return level !== null && RANK[level] >= MIN_RANK[filter];
}

/** A cor da linha: Warden pela origem; erro e fatal; aviso; debug e trace; o resto, info. */
export function lineTone(line: ConsoleLine, level: ConsoleLevel | null): LineTone {
  if (line.origin === 'warden') {
    return 'warden';
  }
  switch (level) {
    case 'error':
    case 'fatal':
      return 'error';
    case 'warn':
      return 'warn';
    case 'debug':
    case 'trace':
      return 'debug';
    case 'info':
    case null:
      return 'info';
  }
}

const MARKS = /\p{M}/gu;

/** Para comparar na busca: minúsculas e sem acentos ("Gráficos" e "graficos" se encontram). */
export function normalizeForSearch(text: string): string {
  return text.normalize('NFD').replace(MARKS, '').toLowerCase();
}

/**
 * Aplica o nível e a busca. `searchable` devolve o texto em que a busca procura (já
 * normalizado com `normalizeForSearch`): o texto mostrado e a origem da linha.
 */
export function filterLines(
  lines: readonly ConsoleLine[],
  filter: ConsoleFilter,
  searchable: (line: ConsoleLine) => string,
): ConsoleRow[] {
  const needle = normalizeForSearch(filter.query.trim());
  const rows: ConsoleRow[] = [];
  let inherited: ConsoleLevel | null = null;
  for (const line of lines) {
    const level: ConsoleLevel | null = line.level ?? inherited;
    inherited = level;
    if (!matchesLevel(level, filter.level)) {
      continue;
    }
    if (needle && !searchable(line).includes(needle)) {
      continue;
    }
    rows.push({ line, level });
  }
  return rows;
}

/**
 * Onde a busca aparece no texto mostrado, para destacar com `<mark>`: pares `[início, fim)` em
 * posições do texto original, comparando sem maiúsculas e sem acentos.
 */
export function highlightRanges(text: string, query: string): [number, number][] {
  const needle = normalizeForSearch(query.trim());
  if (!needle) {
    return [];
  }
  // Texto normalizado e, para cada posição dele, a posição do caractere original.
  let folded = '';
  const origin: number[] = [];
  let index = 0;
  for (const char of text) {
    const piece = normalizeForSearch(char);
    origin.push(...Array.from({ length: piece.length }, () => index));
    folded += piece;
    index += char.length;
  }
  origin.push(index);
  const ranges: [number, number][] = [];
  let from = folded.indexOf(needle);
  while (from !== -1) {
    const to = from + needle.length;
    const start = origin[from] ?? index;
    // O fim é o começo do caractere original seguinte ao último que casou.
    const lastChar = origin[to - 1] ?? index;
    let end = index;
    for (let i = to; i < origin.length; i += 1) {
      const next = origin[i] ?? index;
      if (next > lastChar) {
        end = next;
        break;
      }
    }
    ranges.push([start, end]);
    from = folded.indexOf(needle, to);
  }
  return ranges;
}
