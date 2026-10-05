import type { DiagnosticsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `diagnostics` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a
 * D-02 acrescentou os códigos da análise pós-crash e da redação.
 */
export const diagnostics: Record<DiagnosticsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao analisar os problemas do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  SESSION_NOT_FOUND:
    'Os arquivos deste teste não existem mais ({{path}}); o teste pode ter sido apagado. Rode o teste de novo para ter um log novo.',
  INVALID_REDACTION_RULE:
    'Uma regra para esconder dados pessoais não é válida ({{pattern}}). Corrija a regra e tente de novo.',
};
