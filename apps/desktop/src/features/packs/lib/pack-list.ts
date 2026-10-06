/**
 * Regras puras de Meus packs (T02): nome dos loaders, resultado do último teste, busca e
 * ordenação da lista e o motivo de cada item da higiene (T04). Sem React: testadas à parte.
 */
import type { HygieneCause, HygieneGroup, PackRow } from '../../../lib/ipc/bindings';

const LOADER_NAMES: Record<string, string> = {
  forge: 'Forge',
  neoforge: 'NeoForge',
  fabric: 'Fabric',
  quilt: 'Quilt',
  liteloader: 'LiteLoader',
};

/** "neoforge" → "NeoForge". Chave desconhecida volta como veio. */
export function loaderName(key: string): string {
  return LOADER_NAMES[key] ?? key;
}

/** Resultado do último teste, como a coluna mostra. */
export type LastTestKind = 'never' | 'ok' | 'crashed' | 'unknown';

/**
 * O que o registro guarda em `lastTest` (gravado pela L-04): `null` antes do primeiro teste,
 * `"ok"` quando o jogo abriu e `"crashed"` quando travou.
 */
export function lastTestKind(lastTest: string | null): LastTestKind {
  if (lastTest === null || lastTest === '') return 'never';
  if (lastTest === 'ok') return 'ok';
  if (lastTest === 'crashed') return 'crashed';
  return 'unknown';
}

export type SortOrder = 'modified' | 'name' | 'lastTest';

export const SORT_ORDERS: readonly SortOrder[] = ['modified', 'name', 'lastTest'];

const collator = new Intl.Collator('pt-BR', { sensitivity: 'base', numeric: true });

/** Travou primeiro (pede atenção), depois resultado desconhecido, abriu e nunca testado. */
const TEST_RANK: Record<LastTestKind, number> = { crashed: 0, unknown: 1, ok: 2, never: 3 };

/** Ordena sem mudar a lista recebida. Empate: pelo nome. */
export function sortRows(rows: readonly PackRow[], order: SortOrder): PackRow[] {
  const byName = (a: PackRow, b: PackRow) => collator.compare(a.name, b.name);
  const sorted = [...rows];
  switch (order) {
    case 'name':
      return sorted.sort(byName);
    case 'lastTest':
      return sorted.sort(
        (a, b) =>
          TEST_RANK[lastTestKind(a.lastTest)] - TEST_RANK[lastTestKind(b.lastTest)] || byName(a, b),
      );
    case 'modified':
      return sorted.sort(
        (a, b) =>
          (b.modifiedAtMs ?? Number.NEGATIVE_INFINITY) -
            (a.modifiedAtMs ?? Number.NEGATIVE_INFINITY) || byName(a, b),
      );
  }
}

function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLocaleLowerCase('pt-BR');
}

/** Busca por nome, versão do Minecraft ou loader, sem diferenciar acentos e maiúsculas. */
export function filterRows(rows: readonly PackRow[], query: string): PackRow[] {
  const needle = fold(query.trim());
  if (needle === '') return [...rows];
  return rows.filter((row) =>
    [row.name, row.minecraft ?? '', row.loader ? loaderName(row.loader) : ''].some((field) =>
      fold(field).includes(needle),
    ),
  );
}

/** Chave do texto do motivo de um item da higiene (em `packs.motivo`), com a contagem. */
export type CauseText =
  | { key: `padrao.${HygieneGroup}` }
  | {
      key:
        | 'copia'
        | 'log'
        | 'instalador'
        | 'outroPrograma'
        | 'desativado'
        | 'raizDesconhecida'
        | 'grande'
        | 'link';
    }
  | { key: 'cache'; count: number };

/** Padrões com uma frase mais clara que a do grupo (ARCHITECTURE §6.4). */
const PATTERN_TEXT: Record<string, Exclude<CauseText, { count: number }>['key']> = {
  '*.bak': 'copia',
  '*.old': 'copia',
  '*.log': 'log',
  '*.log.gz': 'log',
  'hs_err_pid*.log': 'log',
  'replay_pid*.log': 'log',
  '/logs/': 'log',
  '/packwiz-installer*.jar': 'instalador',
  '/.packwiz.toml': 'outroPrograma',
  '/packwiz.json': 'outroPrograma',
  '/launcher_profiles*.json': 'outroPrograma',
  '*.disabled': 'desativado',
};

export function causeText(cause: HygieneCause): CauseText {
  switch (cause.kind) {
    case 'pattern': {
      const specific = PATTERN_TEXT[cause.pattern];
      return specific ? { key: specific } : { key: `padrao.${cause.group}` };
    }
    case 'unknownRootItem':
      return { key: 'raizDesconhecida' };
    case 'largeFile':
      return { key: 'grande' };
    case 'cacheLikeFolder':
      return { key: 'cache', count: cause.files };
    case 'symlink':
      return { key: 'link' };
  }
}
