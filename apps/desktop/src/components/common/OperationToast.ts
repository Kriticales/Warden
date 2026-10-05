/**
 * Toast do fim de uma operação longa (T22): "Concluída: Exportar", "Falhou: Publicar versão"
 * com "Ver detalhes", "Cancelada: …". O erro continua na gaveta de Tarefas: o toast nunca é a
 * única cópia (HANDOFF §3).
 */
import i18n from '../../i18n';
import type { OperationSnapshot } from '../../lib/ipc/bindings';
import { appErrorMessage } from '../../lib/ipc/errors';
import { showToast, type ToastInput } from '../ui/toast';

export interface OperationToastOptions {
  /** Nome da operação, já traduzido. */
  name: string;
  /** Abre os detalhes do erro (a gaveta de Tarefas). */
  onShowDetails?: () => void;
}

/** O toast de uma operação terminada, ou `null` se ela ainda está em andamento. */
export function operationToast(
  operation: OperationSnapshot,
  { name, onShowDetails }: OperationToastOptions,
): ToastInput | null {
  switch (operation.state) {
    case 'succeeded':
      return { kind: 'ok', title: i18n.t('toast.ok', { ns: 'tarefas', nome: name }) };
    case 'cancelled':
      return { kind: 'info', title: i18n.t('toast.cancelada', { ns: 'tarefas', nome: name }) };
    case 'failed': {
      const toast: ToastInput = {
        kind: 'danger',
        title: i18n.t('toast.falhou', { ns: 'tarefas', nome: name }),
        ...(operation.error ? { text: appErrorMessage(operation.error) } : {}),
      };
      if (onShowDetails) {
        const label = i18n.t('verDetalhes', { ns: 'tarefas' });
        toast.action = {
          label,
          altText: i18n.t('titulo', { ns: 'tarefas' }),
          onClick: onShowDetails,
        };
      }
      return toast;
    }
    case 'running':
    case 'waitingForLock':
    case 'cancelling':
      return null;
  }
}

/** Mostra o toast de uma operação terminada. Devolve se mostrou. */
export function showOperationToast(
  operation: OperationSnapshot,
  options: OperationToastOptions,
): boolean {
  const toast = operationToast(operation, options);
  if (!toast) {
    return false;
  }
  showToast(toast);
  return true;
}
