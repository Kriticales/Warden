/**
 * Dados das partes da pré-visualização completa (ARCHITECTURE §18): galeria, notas de uma
 * versão (só ao abrir a linha) e dependências da versão escolhida.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId, type SourceId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery } from '../../../lib/ipc/query';

/** A galeria e as notas de versão não mudam enquanto o app está aberto. */
const PREVIEW_STALE_MS = 5 * 60_000;

export const previewKeys = {
  gallery: (packId: PackId, source: SourceId, projectId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'gallery', source, projectId] as const,
  notes: (packId: PackId, source: SourceId, projectId: string, versionId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'notes', source, projectId, versionId] as const,
  dependencies: (packId: PackId, source: SourceId, versionId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'dependencies', source, versionId] as const,
};

export function useProjectGallery(packId: PackId, source: SourceId, projectId: string) {
  return useQuery({
    ...commandQuery(previewKeys.gallery(packId, source, projectId), () =>
      commands.projectGallery(packId, source, projectId),
    ),
    staleTime: PREVIEW_STALE_MS,
  });
}

/** As notas da versão. Só pede quando `enabled` (a linha da versão foi aberta). */
export function useVersionNotes(
  packId: PackId,
  source: SourceId,
  projectId: string,
  versionId: string,
  enabled: boolean,
) {
  return useQuery({
    ...commandQuery(previewKeys.notes(packId, source, projectId, versionId), () =>
      commands.versionNotes(packId, source, projectId, versionId),
    ),
    enabled,
    staleTime: PREVIEW_STALE_MS,
  });
}

/** As dependências da versão escolhida; "Já no pack" muda quando o inventário muda. */
export function useVersionDependencies(packId: PackId, source: SourceId, versionId: string | null) {
  return useQuery({
    ...commandQuery(previewKeys.dependencies(packId, source, versionId ?? ''), () =>
      commands.versionDependencies(packId, source, versionId ?? ''),
    ),
    enabled: versionId !== null,
    staleTime: 0,
  });
}
