/**
 * Arquivos soltos sobre a janela (P1-11; SPEC T06: "Arrastar arquivos `.jar`/`.zip` para a
 * lista inicia Adicionar arquivo local"). O Rust guarda os caminhos e manda só tokens no evento
 * `local-files-dropped`; a tela aberta decide:
 *
 * - na página Adicionar, os arquivos abrem direto na lista de arquivos;
 * - na seção Mods, a página Adicionar abre já com eles ([`takePendingDrop`]).
 */
import { useNavigate } from '@tanstack/react-router';

import { events, type LocalFilesDropped, type PackId } from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';

let pending: LocalFilesDropped | null = null;

/** Os arquivos soltos na seção Mods, para a página Adicionar abrir com eles (uma vez). */
export function takePendingDrop(): LocalFilesDropped | null {
  const taken = pending;
  pending = null;
  return taken;
}

/** Ouve os arquivos soltos enquanto a tela estiver aberta. */
export function useDroppedFiles(handler: (dropped: LocalFilesDropped) => void): void {
  useTauriEvent(events.localFilesDropped, handler);
}

/** Na seção Mods: arquivos soltos abrem a página Adicionar com eles. */
export function useDropToAdd(packId: PackId): void {
  const navigate = useNavigate();
  useDroppedFiles((dropped) => {
    if (dropped.files.length === 0 && dropped.ignored === 0) return;
    pending = dropped;
    void navigate({ to: '/packs/$packId/adicionar', params: { packId } });
  });
}
