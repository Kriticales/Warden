/**
 * "Entrar com o GitHub" em Configurações → Chaves e contas (UI-03; ADR-0059).
 */
export const github = {
  titulo: 'Conta do GitHub',
  texto:
    'Para enviar relatos direto do Warden, sem abrir o navegador. O token fica guardado no mesmo lugar das chaves e é renovado sozinho.',
  indisponivel:
    'O login com o GitHub ainda não foi configurado nesta versão do Warden. Use Abrir no GitHub no relato.',
  entrar: 'Entrar com o GitHub',
  entrando: 'Pedindo o código…',
  codigo: {
    rotulo: 'Código para digitar no GitHub',
    instrucao:
      'Clique em Abrir o GitHub, digite este código na página e aprove o acesso. O código vale {{minutos}} minutos.',
    copiar: 'Copiar código',
    copiado: 'Código copiado',
    naoCopiou: 'Não deu para copiar. Digite o código no GitHub.',
    abrir: 'Abrir o GitHub',
  },
  aguardando: 'Aguardando você aprovar no GitHub…',
  maisDevagar: 'O GitHub pediu para conferir com menos frequência. Continua esperando.',
  cancelar: 'Cancelar',
  cancelado: 'Entrada cancelada.',
  conectado: 'Conectado como {{login}}',
  conectadoAviso: 'Conectado ao GitHub como {{login}}',
  sair: 'Sair do GitHub',
  saindo: 'Saindo…',
  saiu: 'Você saiu do GitHub. O token e o token de renovação foram apagados.',
  falhou: 'A entrada no GitHub não terminou',
  naoAbriu: 'O navegador não abriu',
  carregando: 'Lendo a conta do GitHub…',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    github: typeof github;
  }
}
