/**
 * Regras puras do arquivo do computador (P1-11): qual achado pelo hash oferecer, o texto dos
 * loaders e a frase de cada erro de gravação. Sem React: testadas à parte.
 */
import type { AppError, FileReport, HashMatch, ProjectKind } from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';

/** "Fabric", "Fabric e Quilt", "Forge, NeoForge e Fabric". */
export function loaderList(keys: readonly string[]): string {
  const names = keys.map(loaderName);
  if (names.length <= 1) return names.join('');
  return `${names.slice(0, -1).join(', ')} e ${names[names.length - 1] ?? ''}`;
}

/**
 * O achado para a referência: o primeiro de um tipo aceito pela página (o Modrinth vem antes,
 * pela ordem do backend). Incompatível também serve: o bloqueio aparece antes.
 */
export function referenceMatch(report: FileReport): HashMatch | null {
  return report.matches.find((found) => found.kind !== null) ?? null;
}

/** O tipo inicial do seletor: o do conteúdo, o do achado ou mod. */
export function initialKind(report: FileReport): ProjectKind {
  return report.kind ?? referenceMatch(report)?.kind ?? 'mod';
}

/** Motivos de `INVALID_INPUT` com frase própria (`adicionarArquivo:erros.<motivo>`). */
const REASONS = [
  'changed',
  'nameTaken',
  'incompatible',
  'tooLarge',
  'notFound',
  'unreadable',
  'badName',
  'notFile',
  'expired',
] as const;
export type FileErrorReason = (typeof REASONS)[number];

/** O motivo de um erro de arquivo, se ele tiver frase própria. */
export function fileErrorReason(error: AppError): FileErrorReason | null {
  if (error.code.code !== 'INVALID_INPUT') return null;
  const reason = error.params.reason;
  return REASONS.find((known) => known === reason) ?? null;
}
