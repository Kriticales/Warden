/**
 * Seção Configs do pack (SPEC T12; protótipo `configs`): árvore de arquivos à esquerda, editor
 * de texto à direita. As frases da busca em todas as configs, do formulário e dos padrões
 * entram com a C-05, a C-04 e a C-06.
 */
export const configs = {
  titulo: 'Configs',
  sub: 'Arquivos de ajuste e scripts do pack. Comentários e formatação são preservados.',
  carregando: 'Lendo os arquivos…',
  erroArvore: 'Não foi possível ler os arquivos.',
  origem: {
    rotulo: 'Origem dos arquivos',
    pack: 'Mostrando: arquivos do pack',
    instancia: 'Mostrando: instância de teste',
  },
  arvore: {
    rotulo: 'Arquivos',
    filtro: 'Filtrar por nome de arquivo',
    filtroLimpar: 'Limpar o filtro',
    semResultado: 'Nenhum arquivo com “{{busca}}”.',
    opcoes: 'Opções do jogo ({{nome}})',
    naoSalvo: 'alterações não salvas',
    binario: 'binário, sem edição',
    somenteLeitura: 'só leitura',
    cortada: 'A lista passou do limite de {{max}} arquivos e foi cortada. Use o filtro por nome.',
  },
  vazio: {
    titulo: 'Este pack ainda não tem configs',
    texto:
      'Os mods criam os arquivos de config na primeira vez que o jogo abre. Teste o pack uma vez e eles aparecem aqui.',
    instanciaTitulo: 'Ainda não existe uma instância de teste',
    instanciaTexto:
      'A instância é criada no primeiro teste do pack. Depois dele, as configs do teste aparecem aqui.',
    instanciaSemArquivos: 'A instância de teste não tem configs ainda.',
    semArquivoTitulo: 'Escolha um arquivo',
    semArquivoTexto: 'Selecione um arquivo na lista para ver e editar o conteúdo.',
  },
  instancia: {
    aviso: 'Você está editando a instância de teste, não o pack.',
    avisoTexto:
      'Vale só para este teste. Quando o jogo fechar, a mudança aparece em “O que mudou durante o teste” e você decide se traz para o pack.',
    etiqueta: 'Instância de teste',
  },
  pack: {
    etiqueta: 'Config do pack',
    etiquetaDica: 'Sempre substitui a do jogador ao atualizar',
  },
  editor: {
    area: 'Editor',
    rotuloTexto: 'Conteúdo de {{arquivo}}',
    salvar: 'Salvar',
    salvando: 'Salvando…',
    buscar: 'Buscar e substituir',
    carregandoArquivo: 'Abrindo o arquivo…',
    erroArquivo: 'Não foi possível abrir {{arquivo}}.',
    naoSalvo_one: '{{count}} linha alterada, ainda não salva',
    naoSalvo_other: '{{count}} linhas alteradas, ainda não salvas',
    naoSalvoSemLinhas: 'Alterações não salvas neste arquivo',
    salvo: 'Sem alterações',
    posicao: 'linha {{linha}}, coluna {{coluna}}',
    descartar: 'Desfazer as alterações',
    salvoToast: '{{arquivo}} salvo.',
    semMudanca: '{{arquivo}} já estava igual; nada foi gravado.',
  },
  formato: {
    toml: 'TOML',
    json: 'JSON',
    yaml: 'YAML',
    properties: 'Propriedades',
    javascript: 'JavaScript',
    texto: 'Texto',
  },
  fimDeLinha: {
    lf: 'LF',
    crlf: 'CRLF',
    mixed: 'Fins de linha mistos',
  },
  somenteLeitura: {
    tooLarge:
      'Arquivo acima de 2 MB: abre só para leitura. Edite-o em outro programa e recarregue aqui.',
    tooLargeSemTexto:
      'Arquivo grande demais para mostrar aqui ({{tamanho}}). Abra-o em outro programa.',
    notUtf8:
      'Este arquivo não está em UTF-8. Ele abre só para leitura, e letras fora do padrão aparecem trocadas por �.',
    binary: 'Arquivo binário (como servers.dat): o Warden não mostra nem edita.',
  },
  avisos: {
    serverconfig:
      'Este tipo de config vale por mundo. Para valer em mundos novos, ele precisa estar em defaultconfigs/.',
    reescrita: 'O jogo pode reescrever este arquivo e apagar comentários que você adicionar.',
    neoforge:
      'Valores fora da faixa voltam ao padrão e chaves que o mod não conhece são apagadas quando o jogo abre.',
    options: 'Este arquivo substitui as preferências de quem já joga o pack.',
  },
  salvarDialogo: {
    titulo: 'Salvar {{arquivo}}?',
    sub: 'Só o que você mudou é gravado. O resto do arquivo fica igual, inclusive os comentários.',
    desligar: 'Dá para desligar esta confirmação em Configurações.',
    continuar: 'Continuar editando',
    confirmar: 'Salvar',
  },
  conflito: {
    titulo: 'Este arquivo foi alterado fora do Warden.',
    texto:
      'Outro programa mudou {{arquivo}} depois que você o abriu. Nada foi gravado. Escolha o que fazer antes de salvar.',
    recarregar: 'Recarregar',
    ver: 'Ver diferenças',
    sobrescrever: 'Sobrescrever',
    arquivoApagado: 'O arquivo foi apagado fora do Warden. Recarregue a lista para continuar.',
    verTitulo: 'Diferenças com o arquivo no disco',
    verSub:
      'Vermelho é o que está no disco agora; verde é o que você escreveu. Sobrescrever grava o seu texto.',
    voltar: 'Voltar',
    recarregarConfirmarTitulo: 'Recarregar {{arquivo}}?',
    recarregarConfirmarTexto: 'As suas alterações neste arquivo serão descartadas.',
    recarregarConfirmar: 'Recarregar e descartar',
    sobrescreverTitulo: 'Sobrescrever {{arquivo}}?',
    sobrescreverTexto:
      'O seu texto substitui o que mudou fora do Warden. O que esse outro programa gravou será perdido.',
    sobrescreverConfirmar: 'Sobrescrever o arquivo',
  },
  sair: {
    titulo: 'Sair sem salvar?',
    texto: '{{arquivo}} tem alterações que ainda não foram salvas.',
    salvar: 'Salvar e continuar',
    descartar: 'Descartar alterações',
    ficar: 'Continuar editando',
  },
} as const;

/**
 * Frases do painel de busca e substituição do editor (CodeMirror), por texto original em
 * inglês. Ficam fora do namespace porque as chaves são as frases do próprio CodeMirror.
 */
export const frasesDoEditor: Record<string, string> = {

    Find: 'Buscar',
    Replace: 'Substituir',
    next: 'próximo',
    previous: 'anterior',
    all: 'todos',
    'match case': 'diferenciar maiúsculas',
    regexp: 'expressão regular',
    'by word': 'palavra inteira',
    replace: 'substituir',
    'replace all': 'substituir todos',
    close: 'fechar',
    'current match': 'ocorrência atual',
    'on line': 'na linha',
    'replaced match on line $': 'ocorrência substituída na linha $',
    'replaced $ matches': '$ ocorrências substituídas',
    'Go to line': 'Ir para a linha',
    go: 'ir',
    selection: 'seleção',
    'Folded lines': 'Linhas dobradas',
    'Unfolded lines': 'Linhas abertas',
    to: 'até',
    'folded code': 'trecho dobrado',
    unfold: 'abrir',
    'Fold line': 'Dobrar linha',
    'Unfold line': 'Abrir linha',
};
