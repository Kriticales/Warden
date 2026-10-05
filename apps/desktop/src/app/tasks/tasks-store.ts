/**
 * Estado de interface da gaveta de Tarefas (Zustand: só o que não vem do backend;
 * ARCHITECTURE §18): se está aberta e até quando as falhas já foram vistas.
 */
import { create } from 'zustand';

interface TasksUiState {
  open: boolean;
  /** Fim (ms) da última falha vista; falhas mais novas acendem o indicador. */
  seenUntilMs: number;
  /** Operação com os detalhes do erro abertos na gaveta. */
  expandedId: string | null;
  setOpen: (open: boolean) => void;
  /** Abre a gaveta já com os detalhes de uma operação. */
  showDetails: (operationId: string) => void;
  toggleDetails: (operationId: string) => void;
}

export const useTasksUi = create<TasksUiState>()((set) => ({
  open: false,
  seenUntilMs: 0,
  expandedId: null,
  setOpen: (open) => {
    // Abrir ou fechar a gaveta conta como ter visto as falhas até agora.
    set({ open, seenUntilMs: Date.now() });
  },
  showDetails: (operationId) => {
    set({ open: true, seenUntilMs: Date.now(), expandedId: operationId });
  },
  toggleDetails: (operationId) => {
    set((state) => ({ expandedId: state.expandedId === operationId ? null : operationId }));
  },
}));
