import type { AiErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `ai` (QUALITY §3; ARCHITECTURE §9.5 "Erros e novas tentativas").
 * Criado pela F0-05 só com `INTERNAL`; a D-04 acrescentou os códigos da IA e a D-14, os das
 * propostas; a D-15, o da chave do provedor da conversa (ADR-0060); a D-16 e a D-17, o da chave sem
 * nenhum modelo da regra do provedor. O limite diário nunca
 * promete horário de volta (S-R5-4 §13: o `retryDelay` diário é enganoso).
 */
export const ai: Record<AiErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no Diagnóstico com IA. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  KEY_INVALID:
    'O Google recusou a chave do Gemini. Confira a chave em Configurações → Chaves e contas.',
  RATE_LIMITED:
    'O limite por minuto do plano gratuito continuou mesmo depois de esperar. Espere um pouco e tente de novo.',
  QUOTA_EXCEEDED:
    'O limite diário do plano gratuito deste modelo acabou. Troque de modelo em Configurações (cada modelo tem a sua cota) ou tente de novo mais tarde.',
  TIMEOUT:
    'O Google não respondeu a tempo, nem nas novas tentativas. Confira a internet e tente de novo.',
  BAD_RESPONSE:
    'A resposta da IA veio num formato que o Warden não consegue usar, mesmo depois de pedir de novo. Tente de novo.',
  TOOL_LIMIT:
    'A IA passou do limite de consultas sem dar a resposta final. Tente de novo ou faça uma pergunta mais específica.',
  BILLING:
    'O Google pediu cobrança para esta chave (faturamento desligado ou sem crédito). Confira o plano da sua chave no Google AI Studio.',
  PERMISSION_DENIED: 'Essa consulta não foi permitida nesta conversa.',
  UNAVAILABLE:
    'O Google está sobrecarregado agora e não respondeu nas novas tentativas. Tente de novo daqui a pouco.',
  NO_MODEL:
    'Nenhum modelo do Gemini disponível para a sua chave serve para o diagnóstico. Confira a chave em Configurações.',
  REJECTED:
    'O Google recusou o pedido (código {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  CONVERSATION_NOT_FOUND: 'Essa conversa não existe mais. Ela pode ter sido apagada.',
  CONVERSATION_BUSY: 'A IA ainda está respondendo nesta conversa. Espere a resposta.',
  PREVIEW_EXPIRED:
    'O texto do consentimento não está mais guardado. Clique em Começar conversa de novo.',
  CONVERSATION_UNREADABLE:
    'O arquivo desta conversa está ilegível e não pôde ser aberto. Apague a conversa e comece outra.',
  PROPOSAL_NOT_FOUND: 'Essa proposta não existe mais nesta conversa.',
  PROPOSAL_DECIDED: 'Essa proposta já foi aplicada ou descartada.',
  PROPOSAL_STALE:
    'O pack mudou desde que a diferença foi mostrada. Nada foi gravado; revise a proposta de novo.',
  PROVIDER_KEY_MISSING:
    'Esta conversa começou com {{providerName}} e continua nesse provedor. Para perguntar de novo, informe a chave desse provedor em Configurações → Chaves e contas.',
  PROVIDER_NO_MODEL:
    'A chave informada para {{providerName}} não dá acesso a nenhum dos modelos que o Warden usa com esse provedor. Confira a chave em Configurações → Chaves e contas ou escolha outro provedor de IA.',
};
