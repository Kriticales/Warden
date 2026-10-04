import type { AppErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros do domínio `app`: o que aconteceu + o que fazer (QUALITY §3). */
export const app: Record<AppErrorCode, string> = {
  INTERNAL:
    'Algo deu errado dentro do Warden. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  OPERATION_NOT_FOUND: 'Essa tarefa não existe mais. Abra Tarefas de novo para ver a lista atual.',
  OPERATION_NOT_CANCELLABLE:
    'Essa tarefa está numa etapa que não pode ser interrompida. Espere ela terminar.',
  SETTINGS_INVALID: 'Um valor das configurações foi recusado. Confira o campo e tente de novo.',
  SETTINGS_NEWER_VERSION:
    'As configurações foram salvas por uma versão mais nova do Warden e não podem ser alteradas nesta. Atualize o Warden.',
};
