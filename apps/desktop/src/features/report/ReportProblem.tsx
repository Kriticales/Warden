/**
 * "Relatar problema" no app inteiro (UI-02): o diálogo e o atalho Ctrl+Shift+E, que abre o
 * diálogo de qualquer tela (o F1 fica livre para uma ajuda futura; o Ctrl+Shift+E não é usado
 * pelo WebView2 nem pelo WebKitGTK).
 */
import { useEffect } from 'react';

import { ReportDialog } from './ReportDialog';
import { openReportDialog } from './store';

/** O atalho: Ctrl+Shift+E (a tecla física E, em qualquer layout de teclado). */
export function isReportShortcut(event: KeyboardEvent): boolean {
  return (
    event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && event.code === 'KeyE'
  );
}

export function ReportProblem() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (isReportShortcut(event) && !event.repeat) {
        event.preventDefault();
        openReportDialog();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);
  return <ReportDialog />;
}
