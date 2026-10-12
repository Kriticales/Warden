/**
 * Diálogo "Relatar problema" aberto ou fechado (UI-02). Um diálogo só, aberto pelo botão da
 * barra de título (ou do rodapé, no Linux) e pelo atalho. O último erro mostrado é lido no
 * momento em que o diálogo abre, para o erro de uma pré-visualização não tomar o lugar dele.
 */
import { create } from 'zustand';

import { lastShownError, type ShownErrorText } from '../../lib/ipc/last-error';

interface ReportDialogState {
  open: boolean;
  /** O último erro mostrado quando o diálogo abriu. */
  lastError: ShownErrorText | null;
}

export const useReportDialog = create<ReportDialogState>()(() => ({
  open: false,
  lastError: null,
}));

/** Abre o diálogo, guardando o último erro mostrado até aqui. */
export function openReportDialog(): void {
  if (!useReportDialog.getState().open) {
    useReportDialog.setState({ open: true, lastError: lastShownError() });
  }
}

/** Fecha o diálogo. */
export function closeReportDialog(): void {
  useReportDialog.setState({ open: false });
}
