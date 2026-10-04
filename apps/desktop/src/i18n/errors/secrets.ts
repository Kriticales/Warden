import type { SecretsErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros das chaves e tokens (`secrets`; ARCHITECTURE §14). */
export const secrets: Record<SecretsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao guardar as chaves. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_VALUE:
    'Esse valor não serve como chave ({{reason}}). Copie a chave de novo, sem aspas nem quebras de linha.',
  NOT_CONFIGURED: 'Essa chave ainda não foi configurada. Salve a chave em Configurações.',
  VAULT_UNAVAILABLE:
    'O cofre do Windows não está acessível agora. Tente de novo; se continuar, escolha guardar as chaves no arquivo .env em Configurações.',
  VAULT_FAILED:
    'O cofre do Windows recusou a operação. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  ENV_FILE_UNREADABLE:
    'O arquivo .env das chaves em {{path}} não pôde ser lido. Apague o arquivo e salve as chaves de novo.',
  VERIFY_FAILED: 'A chave foi salva, mas a conferência não bateu. Salve a chave de novo.',
  BACKEND_SWITCH_FAILED:
    'Não foi possível trocar onde as chaves ficam guardadas; elas continuam onde estavam. Tente de novo.',
};
