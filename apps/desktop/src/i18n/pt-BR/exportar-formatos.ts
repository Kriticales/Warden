/**
 * Formatos de outros launchers na seção Exportar (SPEC T19, P1): `.mrpack` e zip da CurseForge.
 * O que o formato perde, o que pede decisão (trocar pelo Modrinth, embutir arquivos de
 * terceiros), o que impede de gerar e o resultado conferido.
 */
export const exportarFormatos = {
  formato: {
    mrpack: {
      titulo: '.mrpack',
      desc: 'Para o app do Modrinth e launchers compatíveis.',
    },
    curseforge: {
      titulo: '.zip da CurseForge',
      desc: 'Para o app da CurseForge. Não guarda o lado dos mods.',
    },
  },
  carregando: 'Conferindo o pack para este formato…',
  erroAnalise: 'Não foi possível conferir o pack para este formato.',
  resumo: {
    titulo: 'O que vai no arquivo',
    referencias_one: '{{count}} mod vai por referência: quem joga baixa o arquivo pela fonte dele.',
    referencias_other:
      '{{count}} mods vão por referência: quem joga baixa os arquivos pela fonte deles.',
    nenhumaReferencia: 'Nenhum mod vai por referência.',
    embutidos_one: '{{count}} arquivo de terceiros vai dentro do arquivo gerado.',
    embutidos_other: '{{count}} arquivos de terceiros vão dentro do arquivo gerado.',
    nenhumEmbutido: 'Nenhum arquivo de terceiros vai dentro do arquivo gerado.',
    configs: 'Configs e outros arquivos do pack vão dentro, na pasta overrides.',
  },
  perdas: {
    titulo: 'O que este formato perde',
    intro: 'O pack no Warden não muda. Só o arquivo gerado fica sem:',
    sideNotKept_one:
      '{{count}} mod só de cliente: a CurseForge não guarda o lado, então o app dela o trata como de cliente e servidor.',
    sideNotKept_other:
      '{{count}} mods só de cliente: a CurseForge não guarda o lado, então o app dela os trata como de cliente e servidor.',
    serverOnlyLeft_one: '{{count}} mod só de servidor fica de fora do arquivo.',
    serverOnlyLeft_other: '{{count}} mods só de servidor ficam de fora do arquivo.',
    optionalTexts_one:
      '{{count}} mod opcional perde o texto e o padrão do opcional; quem joga escolhe pelo próprio launcher.',
    optionalTexts_other:
      '{{count}} mods opcionais perdem o texto e o padrão do opcional; quem joga escolhe pelo próprio launcher.',
    preserve_one:
      '{{count}} arquivo que o jogador poderia manter (não substituir) passa a ser sempre substituído.',
    preserve_other:
      '{{count}} arquivos que o jogador poderia manter (não substituir) passam a ser sempre substituídos.',
    pinned_one: '{{count}} mod fixado em uma versão perde a fixação.',
    pinned_other: '{{count}} mods fixados em uma versão perdem a fixação.',
    updateSources_one:
      '{{count}} mod perde a fonte de atualização (Modrinth ou CurseForge) dentro do arquivo.',
    updateSources_other:
      '{{count}} mods perdem a fonte de atualização (Modrinth ou CurseForge) dentro do arquivo.',
    noAutoUpdate:
      'Quem joga não recebe atualizações pelo link do pack: cada versão nova é um arquivo novo para baixar e importar.',
    configsEverywhere: 'As configs vão sempre para todos os lados, sem separar cliente e servidor.',
  },
  versao: {
    titulo: 'Versão do pack',
    texto:
      'Este formato exige uma versão e o pack.toml não tem. Digite a versão do arquivo (por exemplo, 1.0.0). O pack no Warden não muda.',
    rotulo: 'Versão',
    dica: 'Até 64 caracteres.',
    erro: 'Digite uma versão de até 64 caracteres.',
  },
  decisoes: {
    titulo_one: '{{count}} arquivo pede decisão ou explicação',
    titulo_other: '{{count}} arquivos pedem decisão ou explicação',
    nome: '{{nome}} ({{arquivo}})',
    local: 'Arquivo local',
    link: 'Link direto para {{host}}',
    linkSemHost: 'Link direto',
    curseforge: 'CurseForge',
    modrinth: 'Modrinth',
  },
  troca: {
    obrigatoria:
      'A CurseForge não deixa outros apps distribuírem este mod, mas o mesmo arquivo existe no Modrinth. Ele vai pelo Modrinth, sem o jar dentro do arquivo.',
    opcional:
      'O mesmo arquivo existe no Modrinth (mesmo hash). Trocando, ele vai por referência, sem o jar dentro do arquivo.',
    rotulo: 'Trocar pelo Modrinth',
    destino: 'Projeto {{projeto}}, arquivo {{arquivo}}',
    semTroca: 'Sem a troca, o jar vai dentro do arquivo e conta como arquivo de terceiros.',
  },
  bloqueado: {
    titulo: 'Este mod não pode ir neste formato',
    texto:
      'O autor bloqueou a distribuição por outros apps na CurseForge e o arquivo não existe no Modrinth. Baixe o mod pela página dele e use um formato que não precise do arquivo, ou tire o mod do pack.',
    pagina: 'Abrir a página na CurseForge',
  },
  indisponivel: {
    titulo: 'Este mod saiu da CurseForge',
    texto:
      'O arquivo não existe mais na CurseForge nem no Modrinth. Troque o mod por outra versão em Mods antes de exportar.',
  },
  embutido: {
    texto:
      'O jar vai dentro do arquivo gerado, na pasta overrides. Isso é redistribuir um arquivo de terceiros.',
  },
  confirmar: {
    titulo: 'Arquivos de terceiros dentro do arquivo',
    mrpack:
      'Estes arquivos não têm um endereço que o launcher saiba baixar e vão dentro do .mrpack. Só inclua se a licença de cada um permitir.',
    curseforge:
      'A CurseForge exige aprovação manual de arquivos que não são dela, e eles vão dentro do zip. Só inclua se a licença de cada um permitir.',
    rotulo: 'Tenho permissão para incluir estes arquivos',
  },
  acao: {
    exportar: 'Exportar…',
    dicaMrpack: 'A seguir, você escolhe onde salvar o arquivo .mrpack.',
    dicaCurseforge: 'A seguir, você escolhe onde salvar o arquivo .zip.',
    bloqueado: 'Resolva os mods que não podem ir neste formato antes de exportar.',
    faltaVersao: 'Digite a versão do pack para exportar.',
    faltaConfirmacao: 'Confirme que pode incluir os arquivos de terceiros para exportar.',
    somenteLeitura: 'Este pack está só para leitura: dá para exportar normalmente.',
  },
  andamento: {
    rotulo: 'Exportando',
    meta: 'Gerando o arquivo com o packwiz e conferindo no fim',
    aguardando: 'Aguardando outra operação neste pack',
    cancelar: 'Cancelar',
  },
  falha: 'Não foi possível exportar.',
  resultado: {
    titulo: 'Arquivo exportado',
    resumo: '{{tamanho}}, em',
    trocados_one: '{{count}} mod foi trocado pelo Modrinth.',
    trocados_other: '{{count}} mods foram trocados pelo Modrinth.',
    conferido: 'Arquivo conferido.',
    conferidoTexto:
      'O arquivo abre como zip, o índice do formato é válido e a pasta overrides só tem arquivos do pack.',
    detalhes: {
      entradas_one: '{{count}} entrada no zip',
      entradas_other: '{{count}} entradas no zip',
      referencias_one: '{{count}} mod por referência',
      referencias_other: '{{count}} mods por referência',
      overrides_one: '{{count}} arquivo em overrides',
      overrides_other: '{{count}} arquivos em overrides',
      jars_one: '{{count}} jar embutido',
      jars_other: '{{count}} jars embutidos',
    },
    fechar: 'Fechar',
    mostrarArquivo: 'Mostrar o arquivo',
  },
} as const;
