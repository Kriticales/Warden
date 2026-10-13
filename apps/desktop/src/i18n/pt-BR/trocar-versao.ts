/**
 * Textos de "Trocar de versão", da versão do loader e das dependências sem uso ao remover
 * (SPEC T06, T07 e T11; P1-15).
 */
export const trocarVersao = {
  bloco: {
    botao: 'Trocar de versão…',
    dica: 'Escolha qualquer versão compatível com o pack, inclusive uma mais antiga.',
  },
  dialogo: {
    titulo: 'Trocar a versão de {{name}}',
    descricao:
      'Versões que servem para este pack, da mais nova para a mais antiga. Nada muda até você confirmar.',
    carregando: 'Buscando as versões…',
    versao: 'Versão',
    instalada: 'Instalada agora: {{versao}}',
    opcao: '{{versao}} · {{data}}',
    opcaoComMarcas: '{{versao}} · {{data}} · {{marcas}}',
    marca: {
      instalada: 'instalada',
      maisAntiga: 'mais antiga',
      beta: 'beta',
      alpha: 'alfa',
    },
    semVersoes: 'Nenhuma versão deste item serve para o pack.',
    atualIncompativel:
      'A versão instalada não serve para este pack (outra versão do Minecraft ou outro loader). Escolha uma das versões abaixo.',
    fixado: 'O item está fixado: continua fixado na versão que você escolher.',
    conferindo: 'Conferindo a versão escolhida…',
    mesma: 'É a versão instalada. Escolha outra para trocar.',
    rebaixamentoTitulo: 'Versão mais antiga que a instalada',
    rebaixamento:
      'Você vai voltar de {{atual}} para {{nova}}. Versões antigas podem não ter correções e, em mundos já criados, coisas novas do mod podem sumir. Teste o pack depois de trocar.',
    dependenciasTitulo_one: 'Esta versão precisa de um mod que não está no pack',
    dependenciasTitulo_other: 'Esta versão precisa de {{count}} mods que não estão no pack',
    dependencias: '{{nomes}}. Adicione pela página Adicionar depois de trocar.',
    conflitosTitulo: 'Esta versão diz que não funciona com itens do pack',
    conflitos: '{{nomes}}.',
    cancelar: 'Cancelar',
    trocar: 'Trocar para {{versao}}',
    rebaixar: 'Voltar para {{versao}}',
    trocando: 'Trocando…',
    feito: '{{name}} agora está na versão {{versao}}.',
  },
  loader: {
    botao: 'Trocar versão do loader…',
    titulo: 'Trocar a versão do {{loader}}',
    descricao:
      'Mesmo loader, outra versão. O Warden confere cada mod antes e nada muda até você confirmar.',
    versao: 'Versão do {{loader}}',
    atual: '{{versao}} (atual)',
    recomendada: '{{versao}} (recomendada)',
    instavel: '{{versao}} (instável)',
    carregando: 'Buscando as versões do {{loader}}…',
    conferindo: 'Conferindo os mods com a versão {{versao}}…',
    mesma: 'É a versão atual do pack. Escolha outra para trocar.',
    quebraTitulo_one: '{{count}} mod para de funcionar com a versão {{versao}}',
    quebraTitulo_other: '{{count}} mods param de funcionar com a versão {{versao}}',
    continuaTitulo_one: '{{count}} mod já não funcionava e continua sem funcionar',
    continuaTitulo_other: '{{count}} mods já não funcionavam e continuam sem funcionar',
    voltaTitulo_one: '{{count}} mod volta a funcionar',
    voltaTitulo_other: '{{count}} mods voltam a funcionar',
    exige: '{{name}}: pede {{faixa}}',
    tudoCerto_one: 'O mod conferido funciona com a versão {{versao}}.',
    tudoCerto_other: 'Os {{count}} mods conferidos funcionam com a versão {{versao}}.',
    naoConferidos_one:
      '{{count}} mod ainda não foi baixado e não pôde ser conferido: {{nomes}}. O próximo teste baixa e confere.',
    naoConferidos_other:
      '{{count}} mods ainda não foram baixados e não puderam ser conferidos: {{nomes}}. O próximo teste baixa e confere.',
    cancelar: 'Cancelar',
    trocar: 'Trocar para {{versao}}',
    trocando: 'Trocando…',
    feito: 'Versão do {{loader}} trocada para {{versao}}.',
    feitoComProblemas_one:
      'Versão do {{loader}} trocada para {{versao}}. {{count}} mod não funciona com ela.',
    feitoComProblemas_other:
      'Versão do {{loader}} trocada para {{versao}}. {{count}} mods não funcionam com ela.',
  },
  remover: {
    cadeiaTitulo: 'O que para de funcionar',
    precisaDe: '{{name}}: precisa de {{nomes}}',
    indireto: 'por consequência',
    orfasTitulo_one: '{{count}} dependência fica sem uso',
    orfasTitulo_other: '{{count}} dependências ficam sem uso',
    orfasTexto: 'Nenhum item que fica no pack precisa delas. Marque as que quer remover junto.',
    usadaOpcional: 'Usada só como opcional por {{nomes}}',
    confirmarComOrfas_one: 'Remover {{count}} item',
    confirmarComOrfas_other: 'Remover {{count}} itens',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    trocarVersao: typeof trocarVersao;
  }
}
