/**
 * O que é específico do Modrinth na página Adicionar (ADR-0027): ele é a fonte preferida quando
 * o mesmo projeto está nas duas (informa o lado e tem cache local), e o endereço da página do
 * projeto enquanto a pré-visualização ainda não chegou.
 */
import type { ProjectKind, SourceId, SourceRef } from '../../../lib/ipc/bindings';

/** A fonte padrão do seletor **Fonte**. */
export const PREFERRED_SOURCE: SourceId = 'modrinth';

const PATH: Record<ProjectKind, string> = {
  mod: 'mod',
  resourcePack: 'resourcepack',
  shader: 'shader',
};

/** Página do projeto no Modrinth: `https://modrinth.com/mod/sodium`. */
export function modrinthPageUrl(kind: ProjectKind, slug: string): string {
  return `https://modrinth.com/${PATH[kind]}/${encodeURIComponent(slug)}`;
}

/** A referência padrão de um resultado: a do Modrinth, se houver; senão a primeira. */
export function preferredRef<T extends Pick<SourceRef, 'source'>>(sources: readonly T[]): T | null {
  return sources.find((ref) => ref.source === PREFERRED_SOURCE) ?? sources[0] ?? null;
}
