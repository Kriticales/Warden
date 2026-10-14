/**
 * Console agrupado (SPEC T13 "Mostrar: Linha a linha · Agrupado por mod · Só problemas";
 * protótipo `screens-test.js` e `consoleGroups` do design system; ROADMAP D-09). As linhas do
 * jogo continuam como saíram (sem tradução); aqui só os textos do Warden em volta delas.
 */
export const console = {
  mostrar: 'Mostrar',
  modo: {
    linhas: 'Linha a linha',
    agrupado: 'Agrupado por mod',
    problemas: 'Só problemas',
  },
  rotulo: {
    agrupado: 'Linhas agrupadas por mod',
    problemas: 'Erros, avisos e stack traces do console',
  },
  carregando: 'Agrupando as linhas…',
  esperando: 'Esperando o jogo abrir. As linhas aparecem agrupadas assim que ele começar.',
  vazio: 'Nenhuma linha ainda.',
  semResultado: 'Nenhuma linha encontrada com essa busca.',
  semProblemas: 'Nenhum erro, aviso ou stack trace até agora.',
  erro: 'Não foi possível agrupar as linhas do console.',
  grupo: {
    jogo: 'Jogo e loader',
    desconhecido: 'Desconhecido',
    abrir: 'Mostrar as linhas de {{nome}}',
    fechar: 'Esconder as linhas de {{nome}}',
  },
  contagem: {
    erros_one: '{{n}} erro',
    erros_other: '{{n}} erros',
    avisos_one: '{{n}} aviso',
    avisos_other: '{{n}} avisos',
    repetidas_one: '{{n}} repetida',
    repetidas_other: '{{n}} repetidas',
    linhas_one: '{{n}} linha',
    linhas_other: '{{n}} linhas',
  },
  verNoPack: 'Ver no pack',
  verNoPackDe: 'Ver {{nome}} no pack',
  copiar: 'Copiar',
  copiarDe: 'Copiar as linhas de {{nome}}',
  copiado_one: '{{n}} linha copiada.',
  copiado_other: '{{n}} linhas copiadas.',
  copiadoParcial: '{{n}} de {{total}} linhas copiadas.',
  copiadoParcialTexto: 'As outras repetições ficam só no log completo: use Salvar em arquivo….',
  copiaFalhou: 'Não foi possível copiar as linhas.',
  repeticoes: '×{{vezes}}',
  repeticoesRotulo: 'Repetida {{vezes}} vezes',
  primeiraUltima: 'primeira às {{primeira}}, última às {{ultima}}',
  ruido: 'comum, geralmente inofensivo',
  ruidoDica: 'Aparece em muitos packs e quase nunca causa problema.',
  stack_one: 'stack trace, mais {{n}} linha',
  stack_other: 'stack trace, mais {{n}} linhas',
  maisLinhas_one: 'mais {{n}} linha',
  maisLinhas_other: 'mais {{n}} linhas',
  primeiraLinhaDeMod: 'Primeira linha de mod:',
  lendoStack: 'Lendo as linhas…',
  foraDoTeto_one:
    '{{n}} linha diferente não coube no console agrupado. O log completo está em Salvar em arquivo….',
  foraDoTeto_other:
    '{{n}} linhas diferentes não couberam no console agrupado. O log completo está em Salvar em arquivo….',
  mostrarMais_one: 'Mostrar mais ({{n}} restante)',
  mostrarMais_other: 'Mostrar mais ({{n}} restantes)',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    console: typeof console;
  }
}
