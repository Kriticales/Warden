/**
 * Pontos de extensão da publicação para os jogadores (SPEC T17 e T18), preenchidos pela V-03
 * (`features/github`). **Registro acréscimo-apenas** (ROADMAP §1).
 *
 * - `panel`: a área "Publicação para os jogadores" do Histórico (repositório, visibilidade,
 *   última versão publicada, link do pack com Copiar link e Como os jogadores instalam).
 * - `publishButton`: o botão "Publicar versão X" de uma versão final (no Histórico e no aviso
 *   depois de salvar uma versão final).
 */
import type { ComponentType } from 'react';

import type { PackId, PackRow, SavedVersion } from '../../lib/ipc/bindings';
import { PublicationPanel } from '../github/PublicationPanel';
import { PublishButton } from '../github/PublishButton';

export interface PublicationPanelProps {
  packId: PackId;
  pack: PackRow;
  /** As versões salvas, da mais nova para a mais antiga. */
  versions: readonly SavedVersion[];
}

export interface PublishButtonProps {
  packId: PackId;
  /** A versão final que o botão publica. */
  version: string;
  size?: 'sm' | 'md';
  /** Chamado ao abrir a publicação (o aviso "Versão X salva" fecha para dar lugar a ela). */
  onOpen?: () => void;
}

export interface PublicationSlots {
  panel: ComponentType<PublicationPanelProps>;
  publishButton: ComponentType<PublishButtonProps>;
}

/** Registro acréscimo-apenas: a V-03 trocou as duas linhas. */
export const publicationSlots: PublicationSlots = {
  panel: PublicationPanel,
  publishButton: PublishButton,
};
