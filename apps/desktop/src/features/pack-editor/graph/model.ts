/**
 * Modelo do desenho do grafo (D-11): colunas por nível, posição de cada nó, as ligações que
 * cabem entre colunas vizinhas e as listas da explicação em palavras. Funções puras, testadas
 * sem React.
 */
import type {
  EmbeddedMod,
  FocusEdge,
  FocusNode,
  FocusReport,
  RelationKind,
} from '../../../lib/ipc/bindings';

/** Altura de um nó e espaço entre nós (as do protótipo, `depGraph`). */
export const ROW = 56;
export const GAP = 16;
/** Nós mostrados por coluna; o resto fica na lista em texto. */
export const MAX_PER_COLUMN = 40;

/** Como a ligação é desenhada (nunca só pela cor: cada estilo tem traço próprio e legenda). */
export type EdgeStyle = 'required' | 'optional' | 'incompatible' | 'inferred';

export function edgeStyle(kind: RelationKind): EdgeStyle {
  switch (kind) {
    case 'required':
      return 'required';
    case 'optional':
    case 'recommended':
    case 'suggested':
      return 'optional';
    case 'breaks':
    case 'conflicts':
      return 'incompatible';
    case 'inferred':
      return 'inferred';
  }
}

/** Traço da linha de cada estilo (SPEC T06: cheia, tracejada, vermelha cheia, pontilhada). */
export const DASH: Record<EdgeStyle, string | undefined> = {
  required: undefined,
  optional: '6 4',
  incompatible: undefined,
  inferred: '2 4',
};

export function isNeeded(kind: RelationKind): boolean {
  return kind === 'required' || kind === 'inferred';
}

export function isIncompatible(kind: RelationKind): boolean {
  return kind === 'breaks' || kind === 'conflicts';
}

/** Uma coluna do desenho. */
export interface Column {
  level: number;
  /** Os nós visíveis. */
  nodes: FocusNode[];
  /** Quantos ficaram de fora do desenho (aparecem na lista em texto). */
  hidden: number;
}

/** Uma linha numa calha entre duas colunas vizinhas. */
export interface GutterLine {
  key: string;
  /** Centro vertical do nó na coluna da esquerda e na da direita. */
  y1: number;
  y2: number;
  style: EdgeStyle;
}

export interface FocusLayout {
  columns: Column[];
  /** As linhas de cada calha (uma calha a menos que colunas). */
  gutters: GutterLine[][];
  /** Altura do desenho, em pixels. */
  height: number;
  /** Centro vertical de cada nó visível. */
  y: Map<string, number>;
  /** A ligação que liga cada nó ao lado do centro (dá o tipo mostrado no nó). */
  relation: Map<string, FocusEdge>;
}

/** Centros verticais de `count` nós numa coluna de altura `height`, centralizados. */
function centers(count: number, height: number): number[] {
  const used = count * (ROW + GAP) - GAP;
  const offset = (height - used) / 2;
  return Array.from({ length: count }, (_, index) => offset + index * (ROW + GAP) + ROW / 2);
}

/** Os níveis do desenho: sempre −1, 0 e 1; −2 e 2 quando há nós neles. */
function levelsOf(report: FocusReport): number[] {
  const present = new Set(report.nodes.map((node) => node.level));
  return [-2, -1, 0, 1, 2].filter((level) => Math.abs(level) <= 1 || present.has(level));
}

export function layoutFocus(report: FocusReport, maxPerColumn = MAX_PER_COLUMN): FocusLayout {
  const columns: Column[] = levelsOf(report).map((level) => {
    const all = report.nodes.filter((node) => node.level === level);
    return {
      level,
      nodes: all.slice(0, maxPerColumn),
      hidden: Math.max(0, all.length - maxPerColumn),
    };
  });
  const rows = Math.max(1, ...columns.map((column) => column.nodes.length));
  const height = rows * (ROW + GAP) - GAP;
  const y = new Map<string, number>();
  const columnOf = new Map<string, number>();
  columns.forEach((column, index) => {
    centers(column.nodes.length, height).forEach((center, position) => {
      const node = column.nodes[position];
      if (node) {
        y.set(node.key, center);
        columnOf.set(node.key, index);
      }
    });
  });

  const gutters: GutterLine[][] = columns.slice(1).map(() => []);
  const relation = new Map<string, FocusEdge>();
  for (const edge of report.edges) {
    const from = columnOf.get(edge.from);
    const to = columnOf.get(edge.to);
    if (from === undefined || to === undefined || Math.abs(from - to) !== 1) continue;
    const left = Math.min(from, to);
    const leftKey = from < to ? edge.from : edge.to;
    const rightKey = from < to ? edge.to : edge.from;
    const line: GutterLine = {
      key: `${edge.from}|${edge.to}|${edge.kind}`,
      y1: y.get(leftKey) ?? 0,
      y2: y.get(rightKey) ?? 0,
      style: edgeStyle(edge.kind),
    };
    gutters[left]?.push(line);
    // O nó mais longe do centro leva o tipo da ligação com o vizinho mais perto.
    const levelFrom = columns[from]?.level ?? 0;
    const levelTo = columns[to]?.level ?? 0;
    const outer = Math.abs(levelFrom) > Math.abs(levelTo) ? edge.from : edge.to;
    if (!relation.has(outer)) relation.set(outer, edge);
  }
  return { columns, gutters, height, y, relation };
}

/** Caminho em S entre as duas pontas de uma calha de largura `width`. */
export function gutterPath(line: GutterLine, width: number): string {
  const [y1, y2, middle, end] = [line.y1, line.y2, width / 2, width].map(String);
  return `M0 ${y1 ?? ''} C ${middle ?? ''} ${y1 ?? ''}, ${middle ?? ''} ${y2 ?? ''}, ${end ?? ''} ${y2 ?? ''}`;
}

/** O que a explicação em palavras lista sobre o mod no centro. */
export interface CenterSummary {
  needs: FocusNode[];
  optional: FocusNode[];
  incompatible: FocusNode[];
  neededBy: FocusNode[];
}

export function summarizeCenter(report: FocusReport): CenterSummary {
  const center = report.center?.path;
  const byKey = new Map(report.nodes.map((node) => [node.key, node]));
  const summary: CenterSummary = { needs: [], optional: [], incompatible: [], neededBy: [] };
  const add = (list: FocusNode[], key: string) => {
    const node = byKey.get(key);
    if (node && !list.includes(node)) list.push(node);
  };
  for (const edge of report.edges) {
    if (edge.from === center) {
      if (isNeeded(edge.kind)) add(summary.needs, edge.to);
      else if (isIncompatible(edge.kind)) add(summary.incompatible, edge.to);
      else add(summary.optional, edge.to);
    } else if (edge.to === center) {
      if (isNeeded(edge.kind)) add(summary.neededBy, edge.from);
      else if (isIncompatible(edge.kind)) add(summary.incompatible, edge.from);
    }
  }
  return summary;
}

/** Um nome da lista "Dentro dele": um mod embutido, ou as partes da Fabric API juntas. */
export interface EmbeddedEntry {
  key: string;
  /** O nome declarado; vazio no grupo da Fabric API (o texto vem da tradução). */
  name: string;
  /** Os ids (para o `title`; o id cru não aparece no texto). */
  ids: string[];
  /** Quantas partes da Fabric API o grupo junta (0 fora do grupo). */
  fabricParts: number;
}

/** Ids que não são módulos da Fabric API, apesar do prefixo: a própria Fabric API inteira, o
 * loader e as linguagens (`fabric-language-kotlin`). */
const NOT_FABRIC_PART = /^fabric[_-]?(api|loader)$|^fabric[_-]language[_-]/;

/** `true` para um módulo da Fabric API (ou da Forgified Fabric API): `fabric_api_base`… */
export function isFabricPart(id: string): boolean {
  const lower = id.toLowerCase();
  return /^fabric[_-]/.test(lower) && !NOT_FABRIC_PART.test(lower);
}

/**
 * A lista "Dentro dele" para ler: os nomes declarados, com os módulos da Fabric API juntos numa
 * entrada só ("partes da Fabric API (N)"), no fim da lista.
 */
export function embeddedEntries(embedded: readonly EmbeddedMod[]): EmbeddedEntry[] {
  const parts = embedded.filter((module) => isFabricPart(module.id));
  const entries: EmbeddedEntry[] = embedded
    .filter((module) => !isFabricPart(module.id))
    .map((module) => ({ key: module.id, name: module.name, ids: [module.id], fabricParts: 0 }));
  if (parts.length > 0) {
    entries.push({
      key: 'fabric-api-parts',
      name: '',
      ids: parts.map((module) => module.id),
      fabricParts: parts.length,
    });
  }
  return entries;
}

/** Nome de cada chave de nó. */
export function labels(report: FocusReport): Map<string, string> {
  return new Map(report.nodes.map((node) => [node.key, node.label]));
}

/** Os mods do pack para o seletor do centro, com a busca aplicada. */
export function centerOptions<T extends { name: string }>(
  items: readonly T[],
  search: string,
): T[] {
  const term = search.trim().toLocaleLowerCase('pt-BR');
  const sorted = [...items].sort((a, b) => a.name.localeCompare(b.name, 'pt-BR'));
  return term === ''
    ? sorted
    : sorted.filter((item) => item.name.toLocaleLowerCase('pt-BR').includes(term));
}

/** Acima disso, "Ver o pack inteiro" avisa antes de desenhar (SPEC T06). */
export const WHOLE_WARNING = 150;

/** Para cada nó do pack inteiro, o que ele exige (obrigatória ou inferida), para a lista em texto. */
export function wholeList(report: FocusReport): { node: FocusNode; needs: string[] }[] {
  const names = labels(report);
  return report.nodes.map((node) => ({
    node,
    needs: report.edges
      .filter((edge) => edge.from === node.key && isNeeded(edge.kind))
      .map((edge) => names.get(edge.to) ?? edge.to),
  }));
}
