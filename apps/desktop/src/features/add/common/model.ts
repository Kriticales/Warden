/**
 * Regras puras da página Adicionar (testadas em `model.test.ts`): juntar as páginas da busca,
 * "Já no pack", rótulo da fonte, avisos das fontes e formatos curtos ("48 mi", "há 3 dias").
 */
import type {
  SearchPage,
  SearchResult,
  SourceId,
  SourceRef,
  SourceWarning,
} from '../../../lib/ipc/bindings';

/** Chave de uma referência numa fonte: `modrinth:AANobbMI` (a mesma do inventário). */
export function refKey(ref: Pick<SourceRef, 'source' | 'projectId'>): string {
  return `${ref.source}:${ref.projectId}`;
}

/**
 * Os resultados de todas as páginas, na ordem, sem repetir um projeto que já apareceu por
 * alguma das fontes (o motor pode juntar um item de mais adiante numa página e a fonte
 * entregá-lo de novo depois; ADR-0027).
 */
export function mergePages(pages: readonly SearchPage[]): SearchResult[] {
  const seen = new Set<string>();
  const results: SearchResult[] = [];
  for (const page of pages) {
    for (const item of page.items) {
      const keys = item.sources.map(refKey);
      if (keys.some((key) => seen.has(key))) continue;
      for (const key of keys) seen.add(key);
      results.push(item);
    }
  }
  return results;
}

/** "Já no pack": pela busca ou pelo inventário atual (o que acabou de entrar também conta). */
export function isInPack(result: SearchResult, inventoryKeys: ReadonlySet<string>): boolean {
  return result.inPack || result.sources.some((ref) => inventoryKeys.has(refKey(ref)));
}

/** Qual rótulo de fonte mostrar: uma fonte ou as duas. */
export type SourceLabel = SourceId | 'ambas';

export function sourceLabel(sources: readonly Pick<SourceRef, 'source'>[]): SourceLabel {
  const ids = new Set(sources.map((ref) => ref.source));
  if (ids.size > 1) return 'ambas';
  return ids.has('curseforge') ? 'curseforge' : 'modrinth';
}

/** As fontes consultadas ou avisadas em alguma página (o filtro Fonte só aparece com duas). */
export function knownSources(pages: readonly SearchPage[]): SourceId[] {
  const ids = new Set<SourceId>();
  for (const page of pages) {
    for (const source of page.sources) ids.add(source);
    for (const warning of page.warnings) ids.add(warning.source);
  }
  return (['modrinth', 'curseforge'] as const).filter((id) => ids.has(id));
}

/** Os avisos das páginas, um por fonte e motivo. */
export function pageWarnings(pages: readonly SearchPage[]): SourceWarning[] {
  const found = new Map<string, SourceWarning>();
  for (const page of pages) {
    for (const warning of page.warnings) {
      const key = `${warning.source}:${warning.reason}`;
      if (!found.has(key)) found.set(key, warning);
    }
  }
  return [...found.values()];
}

const compact = new Intl.NumberFormat('pt-BR', { notation: 'compact', maximumFractionDigits: 1 });

/** Contagem curta: 48_000_000 → "48 mi"; 840_000 → "840 mil". */
export function formatCount(value: number | null): string {
  return compact.format(Math.max(0, value ?? 0));
}

/** Quanto tempo faz, em dias, meses ou anos (pelo relógio local). */
export type Ago =
  { unit: 'hoje' } | { unit: 'ontem' } | { unit: 'dias' | 'meses' | 'anos'; count: number };

const DAY_MS = 24 * 60 * 60 * 1000;

export function ago(iso: string, now: number): Ago | null {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) return null;
  const startOf = (ms: number) => {
    const day = new Date(ms);
    day.setHours(0, 0, 0, 0);
    return day.getTime();
  };
  const days = Math.max(0, Math.round((startOf(now) - startOf(time)) / DAY_MS));
  if (days === 0) return { unit: 'hoje' };
  if (days === 1) return { unit: 'ontem' };
  if (days < 30) return { unit: 'dias', count: days };
  if (days < 365) return { unit: 'meses', count: Math.floor(days / 30) };
  return { unit: 'anos', count: Math.floor(days / 365) };
}
