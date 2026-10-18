/**
 * Textos do ✦ Diagnóstico com IA (SPEC T14 ✦; ROADMAP D-04; protótipo
 * `design/prototipo-final/screens-diag.js`). Termos do glossário (QUALITY §8.2): "conversa",
 * "Enviado à IA", "proposta", "travamento", "teste", "chave", "config", "pack". Os textos fixos
 * da SPEC estão iguais aos dela ("Cada coisa que a IA consultar…", "A IA pode errar…",
 * "Aguardando o limite por minuto do plano gratuito…", "O limite diário…" fica na frase do erro).
 */
export const ia = {
  secao: {
    titulo: 'Diagnóstico com IA',
    sub: 'Converse com a IA (Google Gemini) sobre um problema do pack. Ela consulta o pack, os logs e os testes; você vê tudo o que é enviado, e nada muda no pack sem a sua decisão.',
    carregando: 'Lendo as conversas…',
  },
  chave: {
    titulo: 'Falta a chave do Gemini',
    texto:
      'Para usar a IA, informe sua chave do Gemini em Configurações. A chave é sua: o Google pode cobrar pelo uso, conforme o seu plano.',
    abrir: 'Abrir Configurações',
  },
  nova: {
    titulo: 'Nova conversa',
    sintoma: 'O que está acontecendo?',
    fonte: {
      rotulo: 'Começar pelo',
      ultimo: 'Último travamento: {{quando}}',
      sessao: 'Teste de {{quando}} ({{resultado}})',
      arquivo: 'Um log ou crash report do computador…',
      arquivoEscolhido: 'Arquivo do computador: {{arquivo}}',
      arquivoDica:
        '{{tamanho}}. Antes de ir para a IA, o texto passa pela mesma limpeza de dados pessoais dos testes.',
      pack: 'Só o pack, sem log',
    },
    nota: 'Contar algo à IA',
    opcional: '(opcional)',
    notaExemplo: 'Ex.: trava quando entro no Nether',
    comecar: 'Começar conversa',
    preparando: 'Preparando o texto…',
    antes:
      'Antes, você vê o que a IA poderá consultar e confirma. Isso é pedido uma vez por conversa.',
    erro: 'Não foi possível preparar a conversa.',
  },
  sintoma: {
    crashOnStart: { titulo: 'Trava ao abrir o jogo', desc: 'Fecha antes do menu principal.' },
    crashInWorld: {
      titulo: 'Trava ao entrar no mundo',
      desc: 'Abre, mas fecha ao carregar ou jogar.',
    },
    slow: { titulo: 'Está lento', desc: 'Quadros por segundo baixos ou travadinhas.' },
    other: { titulo: 'Outra coisa', desc: 'Conte no campo "Contar algo à IA".' },
  },
  resultado: {
    crashed: 'travou',
    closedNormally: 'fechou normalmente',
    stoppedByUser: 'parado por você',
  },
  modelo: {
    atual: 'Modelo: {{modelo}} ({{nivel}}).',
    consultando: 'Consultando os modelos da sua chave…',
    nenhum: 'Nenhum modelo do Gemini disponível para a sua chave.',
    trocar: 'Mudar em Configurações',
  },
  nivel: {
    economical: 'mais econômico',
    careful: 'análise mais cuidadosa',
  },
  consentimento: {
    titulo: 'Começar uma conversa com a IA?',
    sub: 'Pedido uma vez para esta conversa. Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada. Nada muda no pack sem você clicar em Aplicar.',
    podeConsultar: 'Durante esta conversa, a IA pode consultar',
    pack: 'Informações do pack e a lista de mods',
    packDesc: 'Versões do Minecraft, do loader e do Java; nome, versão e lado de cada mod.',
    achados: 'Os problemas que o Warden já encontrou',
    achadosDesc: 'Os achados de Problemas, com as evidências.',
    logs: 'Logs e crash reports dos seus testes',
    logsDesc:
      'Sempre sem dados pessoais: nome de usuário do Windows nos caminhos, nome do jogador, UUID, IPs, e-mails, nome do computador e qualquer coisa com cara de chave viram marcadores.',
    configs: 'Configs do pack',
    configsDesc: 'Arquivos de config/, defaultconfigs/ e kubejs/. Desmarque se não quiser.',
    github: 'Procurar problemas parecidos nas páginas dos mods no GitHub',
    githubDesc: 'Envia ao GitHub só o nome do mod e algumas palavras do erro. Nunca o log.',
    textoInicial: 'Texto inicial, exatamente como vai',
    textoRotulo: 'Texto inicial que será enviado',
    para: 'Para',
    paraValor: 'Google Gemini ({{modelo}}), com a sua chave',
    tamanho: 'Tamanho inicial',
    tamanhoValor:
      '{{bytes}} de texto; cerca de {{tokens}} com as instruções do Warden (token é a unidade que o Google usa para medir e cobrar)',
    tamanhoSoBytes: '{{bytes}} de texto',
    custo: 'Custo',
    custoValor: {
      economical: 'Entre US$ 0,005 e US$ 0,04 numa conversa típica, no plano pago.',
      careful: 'Entre US$ 0,02 e US$ 0,09 numa conversa típica, no plano pago.',
    },
    plano:
      'O uso é cobrado ou limitado na sua chave. No plano gratuito não há custo, mas o Google pode usar o conteúdo enviado para melhorar os produtos dele, e pessoas podem lê-lo.',
    poucasPorDia:
      'No plano gratuito, a análise mais cuidadosa permite poucas conversas por dia. Se o limite acabar, troque para o modelo mais econômico em Configurações.',
    cancelar: 'Cancelar',
    comecar: 'Começar conversa',
    erro: 'A conversa não começou.',
  },
  conversas: {
    titulo: 'Conversas',
    carregando: 'Lendo as conversas…',
    vazio: 'Nenhuma conversa com a IA neste pack.',
    colunas: {
      conversa: 'Conversa',
      quando: 'Quando',
      situacao: 'Situação',
      acoes: 'Ações',
    },
    situacao: {
      running: 'A IA está respondendo',
      answered: 'Respondida',
      failed: 'Parou com erro',
      interrupted: 'Sem resposta',
    },
    continuar: 'Continuar',
    continuarRotulo: 'Continuar a conversa "{{titulo}}"',
    apagarRotulo: 'Apagar a conversa "{{titulo}}"',
    onde: 'As conversas ficam nos dados do Warden, neste computador. Nunca vão para a pasta do pack nem para o GitHub.',
  },
  apagar: {
    titulo: 'Apagar esta conversa?',
    texto:
      'A conversa e tudo o que foi enviado nela saem deste computador. O que já foi enviado ao Google não volta.',
    confirmar: 'Apagar conversa',
    apagando: 'Apagando…',
  },
  origem: {
    travamento: 'Travamento de {{sessao}}',
    teste: 'Teste de {{sessao}}',
    soPack: 'Só o pack',
    sessaoCurta: '{{dia}} às {{hora}}',
    arquivoCurto: 'arquivo escolhido',
  },
  conversa: {
    voltar: 'Conversas',
    carregando: 'Lendo a conversa…',
    naoAchou: 'Conversa não encontrada',
    sub: 'Conversa sobre {{origem}} · começou {{quando}}',
    textoInicial: 'texto inicial',
    voce: 'Você',
    ia: 'IA',
    suaMensagem: 'Sua mensagem',
    mensagemIa: 'Mensagem da IA',
  },
  enviado: {
    titulo: 'Enviado à IA: {{oQue}}',
    pediu: 'O que a IA pediu',
    pediuRotulo: 'Consulta pedida pela IA',
    exato: 'Exatamente o que foi enviado',
    exatoRotulo: 'Bytes enviados à IA',
    negada: 'não permitido nesta conversa',
    falhou: 'sem dados',
    visaoGeral: 'visão geral do pack',
    lista: 'lista de mods',
    listaFiltrada: 'lista de mods com "{{filtro}}"',
    detalhes: 'detalhes do mod {{mod}}',
    achados: 'problemas que o Warden encontrou',
    testes: 'lista de testes',
    teste: 'resumo do teste de {{sessao}}',
    crashReport: 'crash report do teste de {{sessao}}',
    buscaLog: 'busca por "{{busca}}" no log do teste de {{sessao}}',
    trechoLog: 'trecho do log do teste de {{sessao}}, linhas {{de}}–{{ate}}',
    buscaConfigs: 'busca por "{{busca}}" nas configs',
    config: 'config {{arquivo}}',
    historico: 'versões salvas do pack',
    diferenca: 'o que mudou da versão {{de}} para {{para}}',
    dependentes: 'quem depende do mod {{mod}}',
    porQue: 'por que o mod {{mod}} está no pack',
    outra: 'consulta {{nome}}',
  },
  espera: {
    lendo: 'A IA está lendo. Costuma levar de 10 a 40 segundos; você pode sair desta tela.',
    porMinuto:
      'Aguardando o limite por minuto do plano gratuito… A conversa continua sozinha em cerca de {{segundos}} s.',
    tentando:
      'O Google está sobrecarregado agora. Tentando de novo sozinho ({{tentativa}} de {{total}})…',
  },
  erro: {
    nadaMudou: 'Nada foi alterado no pack. A conversa fica guardada.',
    tentar: 'Tentar de novo',
    configuracoes: 'Abrir Configurações',
  },
  interrompida: {
    titulo: 'A resposta desta pergunta não chegou.',
    texto: 'O Warden fechou antes de a IA responder. Nada foi alterado no pack.',
  },
  resposta: {
    titulo: 'Resposta',
    afirmacoes: 'O que a IA afirma',
    limite: 'A IA chegou ao limite de consultas desta pergunta e respondeu com o que já tinha.',
    semConclusao: 'A IA não encontrou a causa com o que consultou.',
    semConclusaoTexto:
      'Isso é uma resposta válida. A busca do culpado testa o pack por partes até achar o mod que causa o problema.',
    culpado: 'Encontrar o mod culpado',
    mods: 'Mods envolvidos',
    podeErrar: 'A IA pode errar. Confira antes de mudar o pack.',
    podeErrarTexto:
      'Cada selo diz de onde veio o trecho citado, não que a conclusão está certa. Afirmações sem evidência conferida aparecem riscadas, como "não verificado".',
  },
  selo: {
    dica: 'Este trecho existe em {{onde}}. Confira se ele sustenta a frase.',
    dicaJogador:
      'Este trecho é uma mensagem de chat de um jogador ({{onde}}). Existe no log, mas não prova nada sozinho.',
    conferidoRotulo: 'Evidência conferida:',
    jogadorRotulo: 'Mensagem de jogador:',
    naoVerificado: 'não verificado',
    idDesconhecido: 'não verificado: trecho que a IA não recebeu',
    jogador: 'baseado em mensagem de jogador',
    trechoDe: 'Trecho de {{onde}}',
    modsInexistentes: 'Citados pela IA, mas não estão no pack: {{mods}}.',
    log: 'log, linha {{linha}}',
    logSemLinha: 'log',
    crash: 'crash report, linha {{linha}}',
    crashSemLinha: 'crash report',
    config: '{{arquivo}}, linha {{linha}}',
    mod: 'lista de mods: {{mod}}',
    jar: 'metadados do jar de {{mod}}',
    achado: 'achado do Warden',
    teste: 'teste de {{sessao}}',
    versao: 'histórico: {{versao}}',
    grafo: 'dependências de {{mod}}',
    visaoGeral: 'visão geral do pack',
  },
  confianca: {
    titulo: 'Confiança',
    low: 'Confiança baixa',
    medium: 'Confiança média',
    high: 'Confiança alta',
    como: 'Calculada pelo Warden a partir das evidências conferidas, não pelo que a IA diz.',
  },
  proposta: {
    titulo: 'Propostas',
    porque: 'Por quê:',
    manual:
      'Para fazer esta mudança, use as telas de sempre do Warden (Mods, Configs ou Ajustes do teste).',
    tipos: {
      atualizar_mod: 'Atualizar',
      remover_mod: 'Remover',
      voltar_versao_mod: 'Voltar a versão',
      mudar_lado: 'Mudar o lado',
      editar_config: 'Editar config',
      ajustar_teste: 'Ajustes do teste',
      testar_de_novo: 'Testar de novo',
      buscar_culpado: 'Encontrar o mod culpado',
    },
  },
  mudou: {
    titulo: 'O pack mudou desde esta conversa.',
    itens: '{{mudancas}} depois de {{quando}}. As respostas podem não valer mais.',
    outros:
      'As configs ou as informações do pack mudaram depois de {{quando}}. As respostas podem não valer mais.',
    atualizados_one: '{{count}} mod foi atualizado',
    atualizados_other: '{{count}} mods foram atualizados',
    adicionados_one: '{{count}} adicionado',
    adicionados_other: '{{count}} adicionados',
    removidos_one: '{{count}} removido',
    removidos_other: '{{count}} removidos',
    nova: 'Começar nova conversa',
    continuar: 'Continuar mesmo assim',
  },
  perguntar: {
    rotulo: 'Perguntar mais à IA',
    exemplo: 'Ex.: e se eu voltar o Create para a versão anterior?',
    dica: 'A pergunta vai como você escrever, sem passar pela limpeza do log.',
    depoisDoErro: 'Para perguntar outra coisa, a pergunta anterior precisa de uma resposta.',
    enviar: 'Enviar',
  },
  lado: {
    rotulo: 'Sobre esta conversa',
    comecou: 'Começou',
    modelo: 'Modelo',
    tamanho: 'Esta conversa',
    inicial: 'Pedido inicial',
    inicialValor: '{{tokens}}, contados pelo Google',
    requisicoes: 'Requisições',
    podeLer: 'A IA pode ler',
    leituras: {
      pack: 'pack e achados',
      logs: 'logs e crash reports',
      configs: 'configs',
    },
    tudo: 'Tudo o que foi enviado está nos blocos "Enviado à IA", byte a byte.',
    apagar: 'Apagar esta conversa',
  },
  tokens: {
    mil: '~{{valor}} mil tokens',
    unidades: '{{valor}} tokens',
  },
  pedir: {
    semCausa: 'Ou converse com a IA sobre este travamento.',
    comCausa: 'Não parece ser isso?',
    texto:
      'Ela consulta o log, o pack e os testes. Antes de começar, você vê o que ela poderá consultar; nada é enviado sem a sua confirmação.',
    botao: 'Pedir ajuda à IA',
  },
  travamentos: {
    conversar: 'Conversar com a IA',
    conversarRotulo: 'Conversar com a IA sobre o travamento de {{quando}}',
  },
  configuracoes: {
    titulo: 'Modelo da IA (Gemini)',
    economical: {
      titulo: 'Mais econômico (padrão)',
      desc: 'O Flash-Lite mais novo do Gemini: rápido, barato e com cota gratuita folgada.',
    },
    careful: {
      titulo: 'Análise mais cuidadosa',
      desc: 'O Flash mais novo do Gemini. No plano gratuito ele permite poucas conversas por dia.',
    },
    modelo: 'Hoje: {{modelo}}.',
    semChave: 'Os nomes dos modelos aparecem depois de salvar a chave do Gemini.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    ia: typeof ia;
  }
}
