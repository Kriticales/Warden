/**
 * Textos da lista de Travamentos da seção Problemas (SPEC T14; ROADMAP D-06; protótipo
 * `design/prototipo-final/screens-pack.js`, `crashesTable`). Termos do glossário (QUALITY §8.2):
 * "travamento", "teste", "versão", "pack".
 */
export const travamentos = {
  titulo: 'Travamentos',
  subtitulo: 'dos seus testes, agrupados pela causa',
  carregando: 'Lendo os travamentos…',
  erro: 'Não foi possível ler os travamentos.',
  vazio: 'Nenhum teste deste pack travou.',
  colunas: {
    causa: 'Causa',
    vezes: 'Vezes',
    ultima: 'Última vez',
    situacao: 'Situação',
    acoes: 'Ações',
  },
  /** Título da linha quando a análise não deixou um achado. */
  excecao: 'O jogo travou com {{excecao}}',
  /** Com onde quebrou: a entidade, o bloco ou a descrição do crash report. */
  excecaoOnde: 'O jogo travou com {{excecao}} ({{onde}})',
  semDetalhe: 'O jogo travou sem deixar a causa no log',
  vezes_one: 'Travou {{count}} vez',
  vezes_other: 'Travou {{count}} vezes',
  versao_one: 'versão {{versoes}}',
  versao_other: 'versões {{versoes}}',
  /** Muitas versões: só a primeira e a última. */
  faixa: '{{primeira}} a {{ultima}}',
  semVersao: 'sem versão',
  comAlteracoes: '{{versao}} com alterações',
  situacao: {
    resolvido: 'Não voltou a acontecer desde a {{versao}}',
    causaEncontrada: 'Causa encontrada',
    causaNaoEncontrada: 'Causa não encontrada',
  },
  verCausa: 'Ver o que causou',
  verCausaRotulo: 'Ver o que causou o travamento de {{quando}}',
  verVezes_one: 'Ver a {{count}} vez',
  verVezes_other: 'Ver as {{count}} vezes',
  ocultarVezes: 'Ocultar',
  vezesTitulo: 'Cada vez que travou',
  vezesCarregando: 'Lendo as vezes…',
  vez: 'Teste de {{quando}} · {{versao}}',
  abrirVez: 'Abrir o teste de {{quando}}',
  rodape:
    'Travamentos com a mesma causa ficam numa linha só. Ficam os 30 testes mais novos que fecharam normalmente; os que travaram ficam até ocuparem {{limite}}.',
  retencao: {
    pertoTitulo: 'Os travamentos guardados estão perto do limite de {{limite}}',
    perto: 'Quando passarem dele, o Warden pede para apagar os mais antigos.',
    passouTitulo: 'Os travamentos guardados passaram do limite de {{limite}}',
    passou_one: 'O travamento mais antigo pode ser apagado para liberar espaço.',
    passou_other: 'Os {{count}} travamentos mais antigos podem ser apagados para liberar espaço.',
    passouSemApagar:
      'O que passa do limite é o travamento mais novo ou o do teste aberto, que o Warden não apaga.',
    apagar: 'Apagar os mais antigos',
    confirmarTitulo_one: 'Apagar o travamento mais antigo?',
    confirmarTitulo_other: 'Apagar os {{count}} travamentos mais antigos?',
    confirmarTexto:
      'O registro e os logs desses testes saem do computador. Os travamentos mais novos e os testes que fecharam normalmente ficam.',
    confirmar_one: 'Apagar {{count}} travamento',
    confirmar_other: 'Apagar {{count}} travamentos',
    apagando: 'Apagando…',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    travamentos: typeof travamentos;
  }
}
