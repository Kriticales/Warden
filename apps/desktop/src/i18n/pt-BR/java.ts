/**
 * Textos da seção Java de Configurações → Teste e dos motivos da escolha do Java (SPEC T11 e
 * T21; ADR-0029; L-01). Os motivos (`motivo.*`) também servem a Ajustes do teste (P1-08), que
 * mostra "Por que não o Java N?".
 */
export const java = {
  secao: {
    titulo: 'Java',
    descricao:
      'O Warden usa sempre o Java mais novo que cada versão do Minecraft aceita, com a atualização mais recente. Quando não é o mais novo de todos, a tabela explica o porquê.',
    carregando: 'Lendo os Javas instalados…',
    vazio: 'Nenhum Java baixado ainda. O Java certo é baixado no primeiro teste de cada pack.',
  },
  tabela: {
    legenda: 'Javas instalados pelo Warden e os packs que usam cada um',
    colunaJava: 'Java',
    colunaUsadoPor: 'Usado por',
    colunaMotivo: 'Por que esta versão',
    colunaAcoes: 'Ações',
    java: 'Java {{major}}',
    detalhesVersao: '{{version}} · {{source}}',
    fonteTemurin: 'Temurin',
    fonteMojang: 'Mojang',
    naoBaixado: 'Ainda não baixado',
    naoBaixadoDetalhe: 'Será baixado no próximo teste',
    nenhumPack: 'Nenhum pack',
    packComVersao: '{{name}} ({{minecraft}})',
    emJogo: 'Em uso por um jogo aberto',
    substituido: 'Substituído pela versão {{version}}. Será removido quando nenhum jogo o usar.',
    semUso: 'Sem uso. "Remover Javas sem uso" apaga este Java.',
    remover: 'Remover',
    removerRotulo: 'Remover o Java {{version}}',
  },
  motivo: {
    USER_CHOICE: 'Você escolheu este Java em Ajustes do teste.',
    FORGE_LEGACY_JAVA8:
      'O Forge 1.7.10 e 1.12.2 só abre no Java 8. A partir do Java 9 ele trava ao iniciar.',
    FORGE_1165_OLD:
      'O Forge 1.16.5 anterior ao 36.2.26 trava no Java 8u321 ou mais novo. Por isso o teste usa o Java 8 até a atualização 312.',
    NEWEST_PROVEN_FOR_RANGE:
      'O Minecraft {{range}} foi feito para o Java {{major}}. Um Java mais novo ainda não foi provado com essas versões.',
    NEWEST_AVAILABLE: 'O Java mais novo que o Warden usa.',
    FROM_VERSION_JSON: 'Esta versão do Minecraft pede o Java {{required}}.',
    FROM_VERSION_JSON_ANTIGA: 'Esta versão antiga do Minecraft usa o Java 8.',
    porQueNao: 'Por que não o Java {{newest}}?',
    escolhaIndisponivel:
      'O Java escolhido em Ajustes do teste não está mais instalado. O teste vai usar o automático.',
  },
  semDecisao: {
    titulo: 'Packs sem Java definido',
    texto:
      'O Warden não sabe qual Java usar com {{packs}}. Escolha o Java em Ajustes do teste de cada um.',
  },
  pastasQuebradas_one:
    'Há {{count}} pasta de Java incompleta. "Remover Javas sem uso" também a apaga.',
  pastasQuebradas_other:
    'Há {{count}} pastas de Java incompletas. "Remover Javas sem uso" também as apaga.',
  acoes: {
    procurarAtualizacoes: 'Procurar atualizações do Java',
    procurando: 'Procurando atualizações…',
    removerSemUso: 'Remover Javas sem uso',
    removendo: 'Removendo…',
  },
  confirmarRemocao: {
    titulo: 'Remover o Java {{version}}?',
    textoSemPacks: 'Nenhum pack usa este Java. Ele pode ser baixado de novo quando precisar.',
    textoComPacks_one:
      'O pack {{packs}} usa este Java. Ele será baixado de novo no próximo teste desse pack.',
    textoComPacks_other:
      'Os packs {{packs}} usam este Java. Ele será baixado de novo no próximo teste desses packs.',
    confirmar: 'Remover Java',
    cancelar: 'Manter',
  },
  confirmarSemUso: {
    titulo: 'Remover os Javas sem uso?',
    texto_one: '{{count}} Java que nenhum pack usa será apagado.',
    texto_other: '{{count}} Javas que nenhum pack usa serão apagados.',
    textoSoPastas: 'As pastas de Java incompletas serão apagadas.',
    confirmar: 'Remover Javas sem uso',
    cancelar: 'Manter',
  },
  resultado: {
    atualizados_one: '{{count}} Java atualizado.',
    atualizados_other: '{{count}} Javas atualizados.',
    nadaNovo: 'Todos os Javas já estão na atualização mais recente.',
    falhou_one:
      'Não foi possível conferir {{count}} Java agora. A fonte de downloads não respondeu; tente de novo mais tarde.',
    falhou_other:
      'Não foi possível conferir {{count}} Javas agora. A fonte de downloads não respondeu; tente de novo mais tarde.',
    antigoMantido:
      'O Java antigo continua instalado até o jogo aberto fechar, e depois é removido.',
    removidos_one: '{{count}} Java removido.',
    removidos_other: '{{count}} Javas removidos.',
    nadaRemovido: 'Nenhum Java sem uso para remover.',
  },
  etapa: {
    resolve: 'Procurando a versão mais nova do Java',
    download: 'Baixando o Java',
    extract: 'Extraindo o Java',
    validate: 'Conferindo o Java',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    java: typeof java;
  }
}
