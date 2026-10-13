/**
 * Qual versão está sendo publicada agora. O botão "Publicar versão X" só pede; quem mostra os
 * diálogos é o `PublishHost`, montado no cabeçalho do pack (fica na tela em todas as seções),
 * porque o botão some quando a versão passa a "Publicada" e o resultado precisa continuar
 * aberto.
 */
import { create } from 'zustand';

import type { PackId } from '../../lib/ipc/bindings';

export interface PublishRequest {
  packId: PackId;
  version: string;
  /** Quem abriu (o foco volta para ele ao fechar). */
  opener: HTMLElement | null;
}

interface PublishStore {
  request: PublishRequest | null;
  open: (request: PublishRequest) => void;
  close: () => void;
}

export const usePublishStore = create<PublishStore>((set) => ({
  request: null,
  open: (request) => {
    set({ request });
  },
  close: () => {
    set({ request: null });
  },
}));
