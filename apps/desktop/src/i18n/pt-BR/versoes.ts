/**
 * Textos de Salvar versão (SPEC T16) e Histórico (SPEC T17). Glossário: "salvar versão",
 * "versão final", "histórico", "ponto de segurança" (QUALITY §8.2); nunca "commit" ou "git".
 */
export const versoes = {
  historico: {
    titulo: 'Histórico',
    sub: 'Salvar versão guarda o pack só neste computador. Os jogadores só recebem as versões que você publica.',
    carregando: 'Lendo o histórico…',
    erro: 'Não foi possível ler o histórico',
    versoes: 'Versões',
    semVersoes: {
      titulo: 'Nenhuma versão salva ainda',
      texto:
        'Quando o pack estiver como você quer, salve a primeira versão. Dá para voltar a ela quando quiser.',
    },
    contagem_one: '{{count}} alteração não salva',
    contagem_other: '{{count}} alterações não salvas',
    pontosLink: 'Pontos de segurança…',
    pontosDica: 'cópias automáticas feitas antes de ações que apagam ou substituem',
    somenteLeitura: 'Este pack não pode ser alterado agora: {{motivo}}',
  },
  publicacao: {
    titulo: 'Publicação para os jogadores',
    antes:
      'Este pack ainda não foi publicado. Salve uma versão marcada como versão final e publique: os jogadores recebem pelo link do pack.',
    indisponivel: 'A publicação no GitHub ainda não está disponível nesta versão do Warden.',
    publicarVersao: 'Publicar versão {{version}}',
  },
  estado: {
    naoSalvas: 'Não salvas',
    salva: 'Só salva',
    final: 'Versão final · não publicada',
    publicada: 'Publicada',
  },
  agora: 'Agora',
  data: {
    hoje: 'hoje, {{hora}}',
    ontem: 'ontem, {{hora}}',
    outro: '{{data}}, {{hora}}',
  },
  naoSalvas: {
    resumo_one: '{{count}} alteração desde a {{version}}',
    resumo_other: '{{count}} alterações desde a {{version}}',
    resumoInicio_one: '{{count}} alteração desde a criação do pack',
    resumoInicio_other: '{{count}} alterações desde a criação do pack',
    nada: 'Nada mudou desde a versão {{version}}.',
    nadaInicio: 'O pack está como foi criado.',
    salvar: 'Salvar versão',
    descartar: 'Descartar',
    descartarRotulo: 'Descartar a alteração de {{path}}',
  },
  mudancas: {
    plataforma: 'Minecraft e loader',
    adicionados: 'Mods adicionados',
    removidos: 'Mods removidos',
    atualizados: 'Mods atualizados',
    ajustados: 'Mods ajustados',
    outros: 'Resource packs e shaders',
    configs: 'Configs alteradas',
    minecraft: 'Minecraft {{de}} → {{para}}',
    loaderTroca: '{{loader}} {{de}} → {{para}}',
    loaderNovo: '{{loader}} {{para}} adicionado',
    loaderSaiu: '{{loader}} {{de}} removido',
    ajustado: '{{nome}} (ajustes do item)',
    outroAdicionado: 'Adicionado: {{nome}}',
    outroRemovido: 'Removido: {{nome}}',
    outroAtualizado: 'Atualizado: {{nome}}',
    outroAjustado: 'Ajustado: {{nome}}',
    semAlteracoes: 'Nenhuma alteração.',
    soControle:
      'Só arquivos de controle do pack mudaram (índice, versão ou changelog). Nenhum item e nenhuma config.',
  },
  versao: {
    verMudancas: 'Ver mudanças',
    fecharDetalhes: 'Fechar detalhes',
    marcarFinal: 'Marcar como versão final',
    desmarcar: 'Desmarcar versão final',
    marcada: 'A versão {{version}} agora é uma versão final.',
    desmarcada: 'A versão {{version}} deixou de ser versão final. Nenhum arquivo do pack mudou.',
    voltar: 'Voltar para esta versão',
    voltarRotulo: 'Voltar para a versão {{version}}',
    changelog: 'Changelog da versão {{version}}',
    semChangelog: 'Esta versão não tem changelog gravado.',
    verDiferencas: 'Ver diferenças para o estado atual',
    semResumo: 'Sem mudanças registradas',
    resumoAdicionados_one: '{{count}} mod adicionado',
    resumoAdicionados_other: '{{count}} mods adicionados',
    resumoRemovidos_one: '{{count}} removido',
    resumoRemovidos_other: '{{count}} removidos',
    resumoAtualizados_one: '{{count}} atualizado',
    resumoAtualizados_other: '{{count}} atualizados',
    resumoConfigs_one: '{{count}} config alterada',
    resumoConfigs_other: '{{count}} configs alteradas',
    resumoOutros_one: '{{count}} resource pack ou shader',
    resumoOutros_other: '{{count}} resource packs e shaders',
    resumoPlataforma: 'Minecraft ou loader mudou',
  },
  diferencas: {
    titulo: 'Da versão {{version}} até agora',
    descricao:
      'O que mudou no pack desde a versão {{version}}, incluindo o que ainda não foi salvo.',
    fechar: 'Fechar',
    carregando: 'Comparando…',
    igual: 'O pack está igual à versão {{version}}.',
  },
  voltar: {
    titulo: 'Voltar o pack para a versão {{version}}?',
    texto:
      'O pack fica exatamente como estava na {{version}}. Arquivos que não existiam nela são removidos.',
    pontoDeSeguranca_one:
      'O estado de agora, com {{count}} alteração não salva, fica guardado num ponto de segurança. O histórico não é apagado.',
    pontoDeSeguranca_other:
      'O estado de agora, com {{count}} alterações não salvas, fica guardado num ponto de segurança. O histórico não é apagado.',
    pontoDeSegurancaLimpo:
      'O estado de agora fica guardado num ponto de segurança. O histórico não é apagado.',
    depois: 'Depois disso, Salvar versão cria uma versão nova a partir dali.',
    confirmar: 'Voltar para {{version}}',
    voltando: 'Voltando…',
    feito: 'O pack voltou para a versão {{version}}',
    feitoTexto: 'O estado de antes ficou guardado em Pontos de segurança.',
  },
  pontos: {
    titulo: 'Pontos de segurança',
    descricao:
      'Cópias automáticas do pack feitas antes de ações que apagam ou substituem. Recuperar um ponto devolve o pack àquele estado.',
    carregando: 'Lendo os pontos de segurança…',
    vazio: 'Ainda não há pontos de segurança neste pack.',
    recuperar: 'Recuperar',
    recuperarRotulo: 'Recuperar o ponto de segurança de {{data}}',
    fechar: 'Fechar',
    confirmarTitulo: 'Recuperar o ponto de segurança de {{data}}?',
    confirmarTexto:
      'O pack volta ao estado guardado nesse ponto ({{motivo}}). Arquivos que não existiam nele são removidos. O estado de agora vira outro ponto de segurança.',
    confirmar: 'Recuperar ponto',
    recuperando: 'Recuperando…',
    feito: 'Ponto de segurança recuperado',
    feitoTexto: 'O estado de antes ficou guardado em outro ponto de segurança.',
  },
  descartar: {
    titulo: 'Descartar a alteração de {{path}}?',
    textoExistia:
      'O arquivo volta a ser como era na última versão salva. O que você mudou nele desde então se perde.',
    textoNovo:
      'Este arquivo não existia na última versão salva, então ele será apagado. O que está nele agora se perde.',
    confirmar: 'Descartar alteração',
    descartando: 'Descartando…',
    feito: 'Alteração descartada',
  },
  salvar: {
    titulo: 'Salvar versão',
    descricao:
      'Guarda o pack como está agora, neste computador. Dá para voltar a ela quando quiser.',
    carregando: 'Calculando o que mudou…',
    erroPrevia: 'Não foi possível calcular o que mudou',
    numero: 'Número da versão',
    sugerido: 'Sugerido: {{version}}, porque {{motivo}}.',
    usarSugerida: 'Usar {{version}}',
    comoEscolhido: 'Como o número é escolhido',
    regraCorrecao: 'só atualizações de itens e mudanças de config',
    regraMenor: 'mods, resource packs ou shaders novos',
    regraMaior:
      'mudou o Minecraft ou o loader, ou saiu um mod com blocos ou itens nos mundos: os mundos dos jogadores podem perder coisas',
    regraPrimeira: 'A primeira versão salva usa a versão que o pack já tem.',
    regraExemplo: '{{de}} → {{para}}',
    notas: 'Notas',
    opcional: 'opcional',
    notasPlaceholder: 'Ex.: mochilas novas e ajuste de velocidade do Create',
    notasDica: 'Entram no topo do resumo automático.',
    notasLongas: 'As notas passam de {{max}} caracteres. Encurte o texto para salvar.',
    marcarFinal: 'Marcar como versão final',
    marcarFinalDica: 'Pronta para os jogadores. Depois de salvar, dá para publicar.',
    resumo: 'Resumo automático',
    resumoVazio: 'Sem itens nem configs alterados: só arquivos de controle do pack mudaram.',
    atencao:
      'Você removeu mods que podem ter conteúdo nos mundos. Avise quem joga para fazer backup. Isso entra no changelog, na seção Atenção.',
    antes: 'Antes de salvar',
    antesDica: 'Isso não impede salvar: é só para você saber.',
    nadaMudou: 'Nada mudou desde a versão {{version}}.',
    nadaMudouInicio: 'O pack ainda não tem nenhuma alteração para salvar.',
    cancelar: 'Cancelar',
    salvar: 'Salvar versão {{version}}',
    salvando: 'Salvando…',
    verificando: 'Conferindo o número…',
  },
  motivo: {
    primeira: 'é a primeira versão salva e usa a versão que o pack já tem ({{packVersion}})',
    primeiraInvalida:
      'é a primeira versão salva e o pack.toml tem "{{packVersion}}", que não é um número de versão válido',
    minecraft: 'a versão do Minecraft mudou',
    loader: 'o loader mudou',
    removeuMundo_one: 'você removeu {{count}} mod com blocos ou itens que podem estar nos mundos',
    removeuMundo_other:
      'você removeu {{count}} mods com blocos ou itens que podem estar nos mundos',
    geracao_one: 'você adicionou ou removeu {{count}} mod de geração de mundo',
    geracao_other: 'você adicionou ou removeu {{count}} mods de geração de mundo',
    adicionou_one: 'você adicionou {{count}} item (mod, resource pack ou shader)',
    adicionou_other: 'você adicionou {{count}} itens (mods, resource packs ou shaders)',
    atualizou_one: 'você atualizou {{count}} item',
    atualizou_other: 'você atualizou {{count}} itens',
    configs_one: 'você mudou {{count}} config',
    configs_other: 'você mudou {{count}} configs',
    outros: 'houve ajustes no pack',
  },
  salva: {
    titulo: 'Versão {{version}} salva',
    tituloFinal: 'Versão {{version}} salva como versão final',
    texto: 'Ela está guardada neste computador. Os jogadores só recebem quando você publicar.',
    textoSalva: 'Ela está guardada neste computador. Nada foi enviado ao GitHub.',
    fechar: 'Fechar',
    exportar: 'Exportar arquivo',
  },
  cabecalho: {
    salvarVersao: 'Salvar versão',
    alteracoesPalavra_one: 'alteração',
    alteracoesPalavra_other: 'alterações',
    rotulo_one: 'Salvar versão, {{count}} alteração não salva',
    rotulo_other: 'Salvar versão, {{count}} alterações não salvas',
    nada: 'Nada para salvar: o pack está igual à última versão salva.',
    somenteLeitura: 'Este pack não pode ser alterado agora: {{motivo}}',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    versoes: typeof versoes;
  }
}
