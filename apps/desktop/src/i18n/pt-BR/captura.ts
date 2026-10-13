/**
 * "O que mudou durante o teste" (SPEC T15; C-03): a parte do resultado do teste, o aviso do
 * cabeçalho e a pergunta antes de testar de novo com mudanças não revisadas.
 */
export const captura = {
  titulo: 'O que mudou durante o teste',
  resumo_one:
    'Durante o teste, {{count}} arquivo mudou · só mudanças de verdade, sem a reformatação que o jogo faz sozinho. Nada vai para o pack sem você marcar.',
  resumo_other:
    'Durante o teste, {{count}} arquivos mudaram · só mudanças de verdade, sem a reformatação que o jogo faz sozinho. Nada vai para o pack sem você marcar.',
  carregando: 'Lendo o que mudou durante o teste…',
  nada: 'Durante o teste, nada mudou nas configs nem nas opções do jogo.',
  reformatados_one:
    '{{count}} arquivo só foi reformatado pelo jogo (mesmos valores) e não aparece.',
  reformatados_other:
    '{{count}} arquivos só foram reformatados pelo jogo (mesmos valores) e não aparecem.',
  trazidas_one: 'Mudanças deste teste revisadas: {{count}} mudança foi trazida para o pack.',
  trazidas_other: 'Mudanças deste teste revisadas: {{count}} mudanças foram trazidas para o pack.',
  descartadas:
    'Mudanças deste teste descartadas: no próximo teste, a instância de teste volta a ter as configs do pack.',
  grupos: {
    conflito: 'O mesmo arquivo mudou no pack e no jogo',
    packConfig: 'Configs do pack que você mudou no jogo',
    options: 'Opções do jogo (options.txt)',
    worldConfig: 'Configs de mundo',
    mod: 'Mods colocados ou tirados à mão na pasta da instância',
    resource: 'Resource packs e shaders novos',
    datapack: 'Datapacks nos mundos',
    generated_one:
      'Arquivo criado pelos mods ({{count}}) · desmarcado: em geral não precisa ir para o pack',
    generated_other:
      'Arquivos criados pelos mods ({{count}}) · desmarcados: em geral não precisam ir para o pack',
  },
  contagem: '({{count}})',
  opcoes: {
    escondidas_one:
      'A mudança automática do jogo (versão, janela, último servidor) fica escondida.',
    escondidas_other:
      'As {{count}} mudanças automáticas do jogo (versão, janela, último servidor) ficam escondidas.',
    noPack: 'No pack mudou para {{valor}} durante o teste.',
  },
  valor: {
    naoExistia: 'não existia',
    apagada: 'apagada',
    para: 'mudou para',
  },
  item: {
    verDiferenca: 'Ver diferença',
    verDiferencaDe: 'Ver diferença de {{nome}}',
    trazer: 'Trazer {{nome}} para o pack',
    apagado: 'O arquivo foi apagado durante o teste. Se trouxer, ele sai do pack.',
    criado: 'Arquivo novo.',
    semValores: 'O conteúdo mudou.',
    destinoMundo: 'Se trouxer, vai para {{destino}} e vale para todo mundo novo dos jogadores.',
  },
  mod: {
    procurando: 'Procurando {{arquivo}} no Modrinth e na CurseForge…',
    reconhecido: '{{arquivo}} é {{nome}} {{versao}}, do {{fonte}}. O arquivo não é copiado.',
    adicionar: 'Adicionar ao pack pelo {{fonte}}',
    naoReconhecido:
      '{{arquivo}} não foi reconhecido no Modrinth nem na CurseForge. O arquivo não é copiado; para levar ao pack, procure o mod em Adicionar.',
    semBusca:
      'Não foi possível procurar {{arquivo}} agora: confira a internet. O arquivo não é copiado.',
    removido: '{{arquivo}} foi tirado da pasta da instância. Ele volta no próximo teste.',
    datapack: '{{arquivo}}: só informativo nesta versão.',
  },
  fontes: {
    modrinth: 'Modrinth',
    curseforge: 'CurseForge',
  },
  conflito: {
    noPack: 'No pack (você editou depois que o teste começou)',
    noJogo: 'No jogo (mudou durante o teste)',
    legenda: 'O que fazer com {{path}}',
    carregando: 'Lendo as versões do arquivo…',
    manter: 'Manter a do pack',
    manterDesc: 'Descarta a mudança do jogo.',
    usar: 'Usar a do jogo',
    usarDesc: 'Descarta a sua edição no pack.',
    juntar: 'Juntar as duas',
    juntarDesc: 'As chaves mudadas são diferentes: dá para ficar com as duas.',
    editar: 'Editar e combinar',
    editarDesc: 'Abre o editor com as três versões.',
    combinado: 'Combinado à mão: o texto do editor vai para o pack.',
    reabrir: 'Abrir o editor de novo',
  },
  editor: {
    titulo: 'Editar e combinar: {{path}}',
    descricao:
      'O texto começa igual ao do pack. Traga do jogo o que quiser; nada vai para o pack até você clicar em Trazer.',
    rotulo: 'Texto combinado de {{path}}',
    usar: 'Usar este texto',
    cancelar: 'Cancelar',
  },
  diferenca: {
    titulo: 'Diferença de texto: {{path}}',
    descricao: 'Antes: no início do teste. Depois: quando o jogo fechou.',
    semTexto: 'Este arquivo não é texto: a diferença não pode ser mostrada.',
    fechar: 'Fechar',
  },
  acoes: {
    descartar: 'Descartar tudo',
    depois: 'Decidir depois',
    trazer_zero: 'Trazer selecionados para o pack',
    trazer_one: 'Trazer {{count}} selecionado para o pack',
    trazer_other: 'Trazer {{count}} selecionados para o pack',
    trazendo: 'Trazendo para o pack…',
  },
  descartar: {
    titulo: 'Descartar as mudanças deste teste?',
    texto:
      'As configs do pack voltam à versão do pack no próximo teste. Os arquivos criados pelos mods ficam na instância de teste.',
    confirmar: 'Descartar mudanças',
    descartando: 'Descartando…',
  },
  trazido: {
    titulo_one: '{{count}} mudança trazida para o pack',
    titulo_other: '{{count}} mudanças trazidas para o pack',
    texto:
      'Agora elas fazem parte do pack e aparecem em Alterações não salvas, junto com o resto do que mudou desde a última versão salva.',
    continuar: 'Continuar editando',
    salvar: 'Salvar versão',
  },
  aviso: 'Mudanças do teste para revisar',
  pendente: {
    titulo: 'Mudanças do último teste para revisar',
    texto:
      'Durante o último teste, configs do pack mudaram e ainda não foram revisadas. Se testar agora, a cópia do pack desfaz essas mudanças.',
    revisar: 'Revisar agora',
    descartarTestar: 'Descartar e testar',
    cancelar: 'Cancelar',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    captura: typeof captura;
  }
}
