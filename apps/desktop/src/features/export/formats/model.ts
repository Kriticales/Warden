/**
 * Regras da tela dos formatos de outros launchers, sem React: o que impede de gerar, quantos
 * arquivos de terceiros iriam dentro e as escolhas que vão ao backend.
 */
import type { FormatAnalysis, FormatChoices, FormatItem } from '../../../lib/ipc/bindings';

/** Tamanho máximo da versão digitada (o mesmo do backend). */
export const MAX_VERSION_CHARS = 64;

/** O que a pessoa decidiu na tela. */
export interface FormatDecisions {
  /** Mods trocáveis que ela quer manter como estão (o jar iria embutido). Padrão: nenhum. */
  keep: ReadonlySet<string>;
  /** Marcou a permissão para embutir arquivos de terceiros. */
  confirmEmbed: boolean;
  /** Versão digitada (só vale quando o `pack.toml` não tem). */
  version: string;
}

export const NO_DECISIONS: FormatDecisions = {
  keep: new Set(),
  confirmEmbed: false,
  version: '',
};

export function isBlocker(item: FormatItem): boolean {
  return item.outcome.kind === 'blocked' || item.outcome.kind === 'unavailable';
}

/** A troca pelo Modrinth é obrigatória quando a CurseForge não deixa embutir o arquivo. */
export function isSwapRequired(item: FormatItem): boolean {
  return item.outcome.kind === 'swap' && item.outcome.required;
}

/** Se o mod trocável vai ser trocado com as decisões de agora. */
export function isSwapped(item: FormatItem, decisions: FormatDecisions): boolean {
  return item.outcome.kind === 'swap' && (item.outcome.required || !decisions.keep.has(item.path));
}

export function swappedPaths(analysis: FormatAnalysis, decisions: FormatDecisions): string[] {
  return analysis.items.filter((item) => isSwapped(item, decisions)).map((item) => item.path);
}

/** Arquivos de terceiros que iriam dentro: os embutidos e os trocáveis que ela manteve. */
export function embeddedCount(analysis: FormatAnalysis, decisions: FormatDecisions): number {
  return analysis.items.filter(
    (item) =>
      item.outcome.kind === 'embed' ||
      (item.outcome.kind === 'swap' && !isSwapped(item, decisions)),
  ).length;
}

export function versionToUse(analysis: FormatAnalysis, decisions: FormatDecisions): string | null {
  if (analysis.version !== null) return null;
  return decisions.version.trim();
}

export function versionValid(text: string): boolean {
  const trimmed = text.trim();
  return (
    trimmed.length > 0 &&
    Array.from(trimmed).length <= MAX_VERSION_CHARS &&
    // eslint-disable-next-line no-control-regex -- o backend também recusa caracteres de controle
    !/[\u0000-\u001f\u007f-\u009f]/.test(trimmed)
  );
}

/** Por que ainda não dá para gerar, ou `null` se já dá. */
export type Obstacle = 'blocked' | 'version' | 'confirmation';

export function obstacle(analysis: FormatAnalysis, decisions: FormatDecisions): Obstacle | null {
  if (analysis.items.some(isBlocker)) return 'blocked';
  if (analysis.version === null && !versionValid(decisions.version)) return 'version';
  if (embeddedCount(analysis, decisions) > 0 && !decisions.confirmEmbed) return 'confirmation';
  return null;
}

export function toChoices(analysis: FormatAnalysis, decisions: FormatDecisions): FormatChoices {
  const version = versionToUse(analysis, decisions);
  return {
    swap: swappedPaths(analysis, decisions),
    confirmEmbed: decisions.confirmEmbed,
    version: version === null || version === '' ? null : version,
  };
}
