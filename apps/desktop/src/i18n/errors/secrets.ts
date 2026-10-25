import type { SecretsErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros das chaves e tokens (`secrets`; ARCHITECTURE §14). */
export const secrets: Record<SecretsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao guardar as chaves. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_VALUE:
    'Esse valor não serve como chave ({{reason}}). Copie a chave de novo, sem aspas nem quebras de linha.',
  NOT_CONFIGURED: 'Essa chave ainda não foi configurada. Salve a chave em Configurações.',
  VAULT_UNAVAILABLE:
    'O cofre do Windows não está acessível agora. Tente de novo; se continuar, reinicie o computador e copie os detalhes técnicos para registrar o problema.',
  VAULT_FAILED:
    'O cofre do Windows recusou a operação. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  ENV_FILE_UNREADABLE:
    'O arquivo .env antigo das chaves em {{path}} não pôde ser lido. Copie as chaves de lá à mão para Configurações → Chaves e contas e apague o arquivo.',
  VERIFY_FAILED: 'A chave foi salva, mas a conferência não bateu. Salve a chave de novo.',
  BACKEND_SWITCH_FAILED:
    'Não foi possível trocar onde as chaves ficam guardadas; elas continuam onde estavam. Tente de novo.',
  ENV_MIGRATION_FAILED:
    'As chaves do arquivo .env antigo não foram levadas para o cofre do Windows. Nada se perdeu: o arquivo continua lá e o Warden tenta de novo.',
};
