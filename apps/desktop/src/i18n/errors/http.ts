import type { HttpErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `http` (QUALITY §3): conexões com a internet e downloads.
 * Dona: P1-03.
 */
export const http: Record<HttpErrorCode, string> = {
  INTERNAL:
    'Algo deu errado numa conexão com a internet. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  RATE_LIMITED:
    'O servidor {{host}} pediu para esperar antes de receber mais pedidos. Aguarde um minuto e tente de novo.',
  SERVER_ERROR:
    'O servidor {{host}} está com problemas agora (erro {{status}}). Tente de novo em alguns minutos.',
  NOT_FOUND:
    'O servidor {{host}} não encontrou o que foi pedido (erro {{status}}). O arquivo ou a página pode ter sido removido; confira se ainda existe.',
  UNEXPECTED_STATUS:
    'O servidor {{host}} recusou o pedido (erro {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_RESPONSE:
    'O servidor {{host}} respondeu algo que o Warden não entende. Tente de novo mais tarde; se continuar, copie os detalhes técnicos e registre o problema.',
  RESPONSE_TOO_LARGE:
    'A resposta do servidor {{host}} é grande demais e foi recusada. Confira se o link aponta para o arquivo certo.',
  HASH_MISMATCH:
    'O arquivo baixado de {{host}} não é igual ao esperado e foi descartado. Tente baixar de novo; se continuar, o arquivo no servidor pode ter sido trocado.',
  SIZE_MISMATCH:
    'O arquivo baixado de {{host}} veio com o tamanho errado e foi descartado. Tente baixar de novo.',
  INVALID_URL:
    'O link {{url}} não pôde ser usado. Confira se ele começa com http:// ou https://.',
  TOO_MANY_REDIRECTS:
    'O servidor {{host}} redirecionou o pedido vezes demais. Tente de novo mais tarde ou confira o link.',
  INSECURE_REDIRECT:
    'O servidor {{host}} tentou levar o download para uma conexão sem segurança (http), e o Warden recusou. Use outro link para o arquivo.',
};
