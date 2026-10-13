/**
 * Pontos de extensão do resultado do Publicar versão. **Registro acréscimo-apenas** (ROADMAP §1).
 *
 * - `prismInstance`: a "Instância pronta para o Prism" no resultado (SPEC T18, D34; tarefa
 *   E-04, posse `features/github/prism/**`): recebe a versão e a Release criada (o id para anexar
 *   o arquivo, `POST …/releases/{id}/assets`). Vazio até a E-04 preencher.
 */
import type { ComponentType } from 'react';

import type { PackId, Published } from '../../lib/ipc/bindings';

export interface PublishResultSlotProps {
  packId: PackId;
  /** A publicação feita: versão, link do pack, Release (com o id) e notas. */
  published: Published;
}

export interface GithubSlots {
  prismInstance: ComponentType<PublishResultSlotProps> | null;
}

/** Registro acréscimo-apenas: a E-04 troca `prismInstance`. */
export const githubSlots: GithubSlots = {
  prismInstance: null,
};
