import type { GithubErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `github` (QUALITY §3): o login pelo GitHub App e o envio do
 * relato (UI-03; ADR-0059). Cada uma diz o que aconteceu e o que fazer.
 */
export const github: Record<GithubErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao falar com o GitHub. Tente de novo; se continuar, use Abrir no GitHub e copie os detalhes técnicos no relato.',
  LOGIN_UNAVAILABLE:
    'O login com o GitHub ainda não foi configurado nesta versão do Warden. Use Abrir no GitHub no relato.',
  LOGIN_APP_REJECTED:
    'O GitHub recusou o app do Warden ({{reason}}). Confira nas configurações do GitHub App se o Device Flow está ligado e tente de novo.',
  LOGIN_CODE_EXPIRED:
    'O código venceu antes da aprovação. Clique em Entrar com o GitHub para gerar outro.',
  LOGIN_DENIED:
    'O acesso foi recusado na página do GitHub. Clique em Entrar com o GitHub para tentar de novo.',
  NOT_SIGNED_IN:
    'Entre com o GitHub em Configurações → Chaves e contas para enviar direto. Enquanto isso, use Abrir no GitHub.',
  SESSION_EXPIRED:
    'O login do GitHub venceu. Entre de novo em Configurações → Chaves e contas; o relato continua aqui. Se preferir, use Abrir no GitHub.',
  PERMISSION_DENIED:
    'O GitHub não deixou o Warden fazer isso. Confira se o app do Warden está instalado no repositório Warden com permissão de Issues e Contents, ou use Abrir no GitHub.',
  REPOSITORY_NOT_FOUND:
    'O GitHub não encontrou o repositório do Warden, ou o login não tem acesso a ele. Confira se o app do Warden está instalado no repositório Warden, ou use Abrir no GitHub.',
  RATE_LIMITED:
    'O GitHub limitou os pedidos do Warden por um tempo. Espere alguns minutos e clique em Tentar de novo.',
  UNAVAILABLE: 'O GitHub está fora do ar ou com problemas agora. Tente de novo em alguns minutos.',
  REQUEST_REJECTED:
    'O GitHub recusou o pedido (erro {{status}}). Tente de novo; se continuar, use Abrir no GitHub.',
  INVALID_RESPONSE:
    'O GitHub respondeu algo que o Warden não entende. Tente de novo mais tarde ou use Abrir no GitHub.',
};
