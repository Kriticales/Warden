/**
 * Textos do raio-x de mixins (SPEC T07 "O que este mod altera no jogo", T14 aviso "alteram o
 * mesmo ponto do jogo"; D-10). Glossário (QUALITY §8.2): "o que este mod altera no jogo",
 * "alteram o mesmo ponto do jogo", nunca "conflito de mixin" nem "são incompatíveis"; "mixin" só
 * nos nomes técnicos. Protótipo aprovado: `design/prototipo-final/screens-pack.js`
 * (`mods-raio-x`).
 */
export const raioX = {
  titulo: 'O que este mod altera no jogo',
  carregando: 'Lendo o que o mod altera…',
  semJar:
    'O arquivo deste mod ainda não está neste computador. Teste o pack uma vez: o Warden lê os mods copiados para o teste.',
  semAlteracoes: 'Este mod não altera o código do jogo por mixins.',
  resumo_one: 'Altera {{count}} parte do jogo',
  resumo_other: 'Altera {{count}} partes do jogo',
  emConfigs_one: ', em {{count}} config.',
  emConfigs_other: ', em {{count}} configs.',
  compartilhadas_one: '{{count}} também alterada por outros mods.',
  compartilhadas_other: '{{count}} também alteradas por outros mods.',
  desligadas_one:
    '{{count}} alteração está desligada pelas configs do pack ou por outro mod e não conta.',
  desligadas_other:
    '{{count}} alterações estão desligadas pelas configs do pack ou por outros mods e não contam.',
  connector:
    'Mod do Fabric carregado pelo Sinytra Connector: ele usa outros nomes para as partes do jogo, então não foi comparado com os mods nativos do pack.',
  mixinSquared:
    'O pack tem o MixinSquared, que deixa um mod cancelar as alterações de outro: este resumo pode não ser exato.',
  problemasDeLeitura_one: '{{count}} arquivo deste mod não pôde ser lido e ficou de fora.',
  problemasDeLeitura_other: '{{count}} arquivos deste mod não puderam ser lidos e ficaram de fora.',
  maisSobreposicoes_one: 'E mais {{count}} parte também alterada por outros mods.',
  maisSobreposicoes_other: 'E mais {{count}} partes também alteradas por outros mods.',
  verTodas: 'Ver todas as alterações',
  verEmProblemas: 'Ver em Problemas',
  rodape:
    '“Alteram o mesmo ponto” não quer dizer que não funcionam juntos: muitas vezes é de propósito. Este resumo é lido dos arquivos dos mods, sem abrir o jogo.',
  risco: {
    high: 'Risco alto',
    medium: 'Risco médio',
    low: 'Risco baixo',
  },
  /** O tipo de alteração em linguagem simples (o nome técnico vai ao lado). */
  tipo: {
    overwrite: 'troca o método inteiro',
    inject: 'acrescenta código',
    redirect: 'troca uma chamada',
    modifyArg: 'muda um valor passado adiante',
    modifyArgs: 'muda os valores passados adiante',
    modifyVariable: 'muda um valor do método',
    modifyConstant: 'troca um valor fixo',
    wrapOperation: 'envolve uma chamada',
    modifyExpressionValue: 'muda um resultado',
    modifyReturnValue: 'muda o que o método devolve',
    wrapWithCondition: 'pode pular uma chamada',
    modifyReceiver: 'muda o objeto de uma chamada',
    wrapMethod: 'envolve o método inteiro',
  },
  /** Por que o risco é aquele (a regra). */
  regra: {
    twoOverwrites: 'Os dois trocam o método inteiro. Só um vale; o outro é deixado de lado.',
    twoReplacements:
      'Os dois trocam o mesmo trecho do método. Só um vale; o outro é pulado em silêncio.',
    overwriteAndBody:
      'Um troca o método inteiro e o outro mexe no meio dele: o trecho que o outro procura pode não existir mais.',
    redirectAndOther:
      'Um troca uma chamada e o outro mexe na mesma chamada. O resultado depende de qual aplica primeiro.',
    twoValueChanges:
      'Os dois mudam o mesmo valor, um depois do outro. Costuma funcionar, mas o resultado final pode surpreender.',
    redirectAndInject:
      'Um troca uma chamada e o outro só acrescenta código nela. O jogo aplica os dois.',
    overwriteAndEdge:
      'Um troca o método inteiro e o outro acrescenta código no começo ou no fim. O jogo aplica os dois.',
    coexist: 'Só acrescentam código. O jogo aplica todos.',
  },
  /** Por que o risco desceu. */
  rebaixador: {
    allPlugin: 'Pode ser desligado pelo próprio mod.',
    compatPackage:
      'Compatibilidade intencional: a alteração de {{nome}} é feita para conviver com outros mods.',
    referencesOther: 'Compatibilidade intencional: {{nome}} usa partes de {{outro}}.',
    declaresDependency: 'Compatibilidade intencional: {{nome}} declara {{outro}} como dependência.',
    pluginCitesOther: 'Feitos para conviver: {{nome}} cita {{outro}} para se ajustar a ele.',
    knownCompatible: 'Par conhecido como compatível.',
  },
  /** Por que o risco subiu (ou o que agrava). */
  elevador: {
    knownConflict: 'Está na lista de conflitos conhecidos do Warden.',
    crash: 'O último teste travou citando {{config}} ({{arquivo}}, linha {{linha}}).',
    require: 'Se o ponto sumir, o jogo trava ao abrir ({{config}} exige a alteração).',
  },
  todas: {
    titulo: 'Todas as alterações de {{nome}}',
    voltar: 'Detalhes do mod',
    resumo_one: '{{count}} alteração',
    resumo_other: '{{count}} alterações',
    configs: 'Configs: {{lista}}.',
    prioridade:
      'Ordem prevista: prioridade {{lista}} (menor aplica antes; a maioria dos mods usa 1000).',
    filtrar: 'Filtrar por parte do jogo',
    mostrar: 'Mostrar',
    mostrarOpcoes: {
      todas: 'Todas as partes',
      compartilhadas: 'Também alteradas por outros mods',
      high: 'Só risco alto',
      medium: 'Só risco médio',
      low: 'Só risco baixo',
    },
    colunas: {
      parte: 'Parte do jogo',
      tipo: 'Tipo',
      outros: 'Outros mods',
    },
    nenhum: 'nenhum',
    vazio: 'Nenhuma parte com esse filtro.',
    mais_one: '… e mais {{count}} parte. Use o filtro para achar.',
    mais_other: '… e mais {{count}} partes. Use o filtro para achar.',
    comoLer: 'Como ler os tipos',
    comoLerTexto:
      '@Inject acrescenta código num ponto e convive bem com outros. @Redirect e @Overwrite trocam código: quando dois mods fazem isso no mesmo ponto, só um vale.',
  },
  /** O aviso da seção Problemas (`diagnostico:regras.W_MIXIN_OVERLAP`). */
  aviso: {
    risco: {
      alto: 'risco alto',
      medio: 'risco médio',
    },
    maisPartes_zero: '',
    maisPartes_one: ' e mais {{count}} parte',
    maisPartes_other: ' e mais {{count}} partes',
    travamento: {
      sim: ' O último teste travou citando a config de um deles.',
      nao: '',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    raioX: typeof raioX;
  }
}
