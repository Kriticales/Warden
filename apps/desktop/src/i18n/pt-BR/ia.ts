/**
 * Textos do Sensor Sculk, a seção de IA do pack (SPEC T14; ROADMAP D-04; protótipo aprovado
 * `design/bug-01/sensor-sculk.html`, versão 2; ADR-0064). Dentro do app a IA se chama **Sculk**.
 * Termos do glossário (QUALITY §8.2): "Sensor Sculk", "Sculk", "conversa", "Enviado ao Sculk",
 * "proposta", "travamento", "teste", "chave", "config", "pack". Os textos fixos da SPEC estão
 * iguais aos dela ("Cada coisa que o Sculk consultar…", "O Sculk pode errar…", "Aguardando o
 * limite por minuto do plano gratuito…").
 */
export const ia = {
  secao: {
    titulo: 'Sensor Sculk',
    carregando: 'Lendo as conversas…',
  },
  provedores: {
    gemini: {
      nome: 'Gemini',
      completo: 'Google Gemini',
      desc: 'Do Google. Tem plano gratuito pela API, com limite de uso por dia.',
    },
    openai: {
      nome: 'OpenAI',
      completo: 'OpenAI',
      desc: 'Pago por uso, cobrado na sua conta da OpenAI.',
    },
    anthropic: {
      nome: 'Claude',
      completo: 'Claude',
      desc: 'Da Anthropic. Pago por uso, cobrado na sua conta da Anthropic.',
    },
    sugerido: 'Sugerido',
  },
  chave: {
    titulo: {
      gemini: 'Falta a chave do Gemini',
      openai: 'Falta a chave da OpenAI',
      anthropic: 'Falta a chave do Claude',
    },
    texto:
      'Para conversar com o Sculk, salve a chave do provedor escolhido em Configurações → Chaves e contas. A chave é sua: o uso é cobrado ou limitado nela, conforme o plano.',
    provedores:
      'O Sculk funciona com o Gemini (Google), a OpenAI e o Claude (Anthropic). Para começar, o sugerido é o Gemini, que tem plano gratuito.',
    abrir: 'Abrir Configurações',
  },
  lista: {
    titulo: 'Conversas',
    nova: 'Nova conversa',
    busca: 'Buscar nas conversas',
    limpar: 'Limpar busca',
    carregando: 'Lendo as conversas…',
    vazio: 'Nenhuma conversa com o Sculk neste pack.',
    vazioDica:
      'Comece uma em Nova conversa. Antes de enviar qualquer coisa, você vê o que o Sculk poderá consultar.',
    semResultado: 'Nenhuma conversa com “{{busca}}”.',
    grupos: {
      hoje: 'Hoje',
      ontem: 'Ontem',
      semana: 'Esta semana',
      antes: 'Antes',
    },
    rotulo: '{{titulo}}, {{origem}}, {{estado}}',
    rodape: {
      provedor: 'Provedor das conversas novas:',
      keyring: 'chave salva no cofre do Windows.',
      envfile: 'chave salva no arquivo .env do Warden.',
      semChave: 'sem chave salva.',
      mudar: 'Mudar em Configurações',
    },
    onde: 'As conversas ficam nos dados do Warden, neste computador. Nunca vão para a pasta do pack nem para o GitHub.',
  },
  estado: {
    lendo: 'Lendo',
    respondida: 'Respondida',
    esperando: 'Esperando você',
    aplicada: 'Aplicada',
    erro: 'Parou com erro',
    interrompida: 'Sem resposta',
  },
  modo: {
    grupo: 'Como o Sculk procura',
    rotulo: 'Como procurar',
    quick: 'Procurar rápido',
    deep: 'Procurar a fundo',
    dica: {
      quick: 'Mais rápido e mais barato: modelo mais econômico e menos consultas.',
      deep: 'Modelo mais forte e mais consultas. Demora mais e custa mais na sua chave.',
    },
    hint: {
      quick: 'Modelo mais econômico e menos consultas. Bom para dúvidas simples.',
      deep: 'Modelo mais forte e mais consultas. Demora mais e custa mais na sua chave.',
    },
    selecionado: '{{modo}} selecionado.',
    provedor: 'Provedor: {{provedor}}. Modelo: {{modelo}}.',
    provedorSemModelo: 'Provedor: {{provedor}}.',
    consultando: 'Consultando os modelos da sua chave…',
    gemini:
      'No Gemini o Sculk usa sempre o mesmo modelo; se o limite diário acabar, troca sozinho para um mais leve e avisa.',
  },
  nova: {
    titulo: 'Nova conversa',
    sub: 'O Sculk lê o travamento, o pack e os testes e explica o que aconteceu. Tudo o que ele consultar aparece na conversa.',
    sintoma: 'O que está acontecendo?',
    sintomaDica: 'Escolha uma opção para começar.',
    fonte: {
      rotulo: 'Começar pelo',
      ultimo: 'Último travamento: {{quando}}',
      sessao: 'Teste de {{quando}} ({{resultado}})',
      arquivo: 'Um log ou crash report do computador…',
      arquivoEscolhido: 'Arquivo do computador: {{arquivo}}',
      arquivoDica: '{{tamanho}}.',
      pack: 'Só o pack, sem log',
      dica: 'O texto passa pela limpeza de dados pessoais antes de ir para o Sculk.',
    },
    nota: 'Conte o que aconteceu',
    opcional: '(opcional)',
    notaExemplo: 'Ex.: trava quando entro no Nether',
    comecar: 'Começar conversa',
    comecarDica: 'Escolha o que está acontecendo para começar.',
    preparando: 'Preparando o texto…',
    antes:
      'Antes, você vê o que o Sculk poderá consultar e confirma. Isso é pedido uma vez por conversa.',
    erro: 'Não foi possível preparar a conversa.',
  },
  sintoma: {
    crashOnStart: { titulo: 'Trava ao abrir o jogo', desc: 'Fecha antes do menu principal.' },
    crashInWorld: {
      titulo: 'Trava ao entrar no mundo',
      desc: 'Abre, mas fecha ao carregar ou jogar.',
    },
    slow: { titulo: 'Está lento', desc: 'Quadros por segundo baixos ou travadinhas.' },
    other: { titulo: 'Outra coisa', desc: 'Conte no campo abaixo o que está acontecendo.' },
  },
  resultado: {
    crashed: 'travou',
    closedNormally: 'fechou normalmente',
    stoppedByUser: 'parado por você',
  },
  consentimento: {
    titulo: 'Começar uma conversa com o Sculk?',
    sub: 'Pedido uma vez para esta conversa. Confira o que o Sculk poderá consultar e o texto que vai primeiro.',
    podeConsultar: 'O que o Sculk poderá consultar',
    sempre: 'Sempre',
    pack: 'Informações do pack e a lista de mods',
    packDesc: 'Versões do Minecraft, do loader e do Java; nome, versão e lado de cada mod.',
    achados: 'Os problemas que o Warden já encontrou',
    achadosDesc: 'Os achados de Problemas, com as evidências.',
    logs: 'Logs e crash reports dos seus testes',
    logsDesc:
      'Sem dados pessoais: nome de usuário do Windows nos caminhos, nome do jogador, UUID, IPs, e-mails, nome do computador e qualquer coisa com cara de chave viram marcadores.',
    desempenho: 'Medidas de desempenho dos testes',
    desempenhoDesc:
      'Tempo para abrir e para entrar no mundo, pico de memória, o perfil de desempenho quando houver e a comparação entre versões. O computador aparece só como um código.',
    configs: 'Configs do pack',
    configsDesc: 'Arquivos de config/, defaultconfigs/ e kubejs/. Desmarque se não quiser.',
    github: 'Procurar problemas parecidos nas páginas dos mods no GitHub',
    githubDesc: 'Envia ao GitHub só o nome do mod e algumas palavras do erro. Nunca o log.',
    textoInicial: 'Texto inicial, exatamente como vai',
    textoRotulo: 'Texto inicial que será enviado',
    para: 'Para',
    paraValor: '{{provedor}} ({{modelo}}), com a sua chave',
    tamanho: 'Tamanho inicial',
    tamanhoValor: '{{bytes}} de texto; cerca de {{tokens}} com as instruções do Warden',
    tamanhoSoBytes: '{{bytes}} de texto',
    token: 'Token é a unidade que os provedores de IA usam para medir e cobrar.',
    custo: 'Custo',
    /** O custo do Gemini pelo modelo (ADR-0055: o Flash-Lite, `leve`; o Flash, `forte`). */
    custoValor: {
      leve: 'Entre US$ 0,005 e US$ 0,04 numa conversa típica, no plano pago.',
      forte: 'Entre US$ 0,02 e US$ 0,09 numa conversa típica, no plano pago.',
    },
    custoPago: {
      openai:
        'O uso é cobrado na sua chave da OpenAI, conforme o tamanho da conversa e os preços dela.',
      anthropic:
        'O uso é cobrado na sua chave da Anthropic, conforme o tamanho da conversa e os preços dela.',
    },
    plano:
      'O uso é cobrado ou limitado na sua chave. No plano gratuito não há custo, mas o Google pode usar o conteúdo enviado para melhorar os produtos dele, e pessoas podem lê-lo.',
    poucasPorDia:
      'No plano gratuito, este modelo permite poucas conversas por dia. Se o limite diário acabar no meio de uma pergunta, o Warden troca sozinho para um modelo mais leve e avisa.',
    aviso:
      'Cada coisa que o Sculk consultar aparece na conversa. Nada muda no pack sem você clicar em Aplicar.',
    copiar: 'Copiar',
    cancelar: 'Cancelar',
    comecar: 'Enviar e começar',
    erro: 'A conversa não começou.',
  },
  apagar: {
    titulo: 'Apagar esta conversa?',
    texto:
      'Ela sai da lista e o arquivo dela é apagado dos dados do Warden. O pack não muda. O que já foi enviado ao Sculk não volta.',
    confirmar: 'Apagar conversa',
    apagando: 'Apagando…',
    cancelar: 'Cancelar',
    apagada: 'Conversa apagada.',
  },
  origem: {
    travamento: 'Travamento de {{sessao}}',
    teste: 'Teste de {{sessao}}',
    soPack: 'Só o pack',
    sessaoCurta: '{{dia}} às {{hora}}',
    arquivoCurto: 'arquivo escolhido',
    link: 'Link do {{site}}',
    sobre: {
      travamento: 'o travamento de {{sessao}}',
      teste: 'o teste de {{sessao}}',
      soPack: 'o pack, sem log',
      arquivo: 'o arquivo {{arquivo}}',
      link: 'o log do link do {{site}}',
    },
  },
  conversa: {
    carregando: 'Lendo a conversa…',
    naoAchou: 'Conversa não encontrada',
    meta: '{{provedor}} · começou {{quando}}',
    inicio: 'Conversa sobre {{origem}}',
    textoInicial: 'Texto inicial',
    voce: 'Você',
    sculk: 'Sculk',
    suaMensagem: 'Sua mensagem',
    mensagemSculk: 'Resposta do Sculk',
    exato: 'Exatamente o que foi enviado ao Sculk · {{bytes}}',
    copiar: 'Copiar',
    copiado: 'Copiado.',
    copiarFalhou: 'Selecione o texto e use Ctrl+C.',
    lendo: 'Sculk lendo…',
    segundos: '{{segundos}} s',
    lendoAnuncio: 'O Sculk está lendo.',
    recebida: 'Resposta do Sculk recebida.',
  },
  consultas: {
    titulo_one: 'O Sculk consultou {{count}} coisa',
    titulo_other: 'O Sculk consultou {{count}} coisas',
    enviados: '{{tamanho}} enviados',
    pre: 'Enviado ao Sculk:',
    pedido: 'Pedido do Sculk',
    oQue: 'O que foi enviado · {{tamanho}}',
    negada: 'não permitido nesta conversa',
    falhou: 'sem dados',
  },
  enviado: {
    titulo: 'Enviado ao Sculk: {{oQue}}',
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
    buscaLink: 'busca por "{{busca}}" no log do link',
    trechoLink: 'trecho do log do link, linhas {{de}}–{{ate}}',
    buscaConfigs: 'busca por "{{busca}}" nas configs',
    config: 'config {{arquivo}}',
    historico: 'versões salvas do pack',
    diferenca: 'o que mudou da versão {{de}} para {{para}}',
    dependentes: 'quem depende do mod {{mod}}',
    porQue: 'por que o mod {{mod}} está no pack',
    desempenho: 'medidas de desempenho dos testes',
    perfilDesempenho: 'perfil de desempenho do teste de {{sessao}}',
    outra: 'consulta {{nome}}',
  },
  espera: {
    porMinuto:
      'Aguardando o limite por minuto do plano gratuito… A conversa continua sozinha em cerca de {{segundos}} s.',
    porMinutoPago:
      'Aguardando o limite por minuto da sua chave… A conversa continua sozinha em cerca de {{segundos}} s.',
    tentando: {
      gemini:
        'O Google está sobrecarregado agora. Tentando de novo sozinho ({{tentativa}} de {{total}})…',
      openai:
        'A OpenAI está sobrecarregada agora. Tentando de novo sozinho ({{tentativa}} de {{total}})…',
      anthropic:
        'A Anthropic está sobrecarregada agora. Tentando de novo sozinho ({{tentativa}} de {{total}})…',
    },
    sair: 'Você pode sair desta tela; a conversa continua.',
  },
  troca: {
    quota: 'O limite diário do modelo mais forte acabou; esta resposta usou o mais leve.',
    timeout: 'O modelo mais forte demorou demais para responder; esta resposta usou o mais leve.',
  },
  faltaChave: {
    texto:
      'Esta conversa começou com {{provedor}} e continua nesse provedor. Para perguntar de novo, salve a chave dele em Configurações → Chaves e contas.',
  },
  erro: {
    nadaMudou: 'Nada foi alterado no pack. A conversa fica guardada.',
    tentar: 'Tentar de novo',
    configuracoes: 'Abrir Configurações',
  },
  interrompida: {
    titulo: 'A resposta desta pergunta não chegou.',
    texto: 'O Warden fechou antes de o Sculk responder. Nada foi alterado no pack.',
  },
  resposta: {
    limite:
      'O Sculk chegou ao limite de consultas desta pergunta e respondeu com o que já tinha.',
    semConclusao: 'O Sculk não encontrou a causa com o que consultou.',
    semConclusaoTexto:
      'Isso é uma resposta válida. A busca do culpado testa o pack por partes até achar o mod que causa o problema.',
    culpado: 'Encontrar o mod culpado',
    semConclusaoSemPack:
      'Isso é uma resposta válida. Conte mais ao Sculk ou analise o log dentro do pack, onde ele também vê os mods e os testes.',
    mods: 'Mods envolvidos',
    podeErrar: 'O Sculk pode errar. Confira antes de mudar o pack.',
  },
  selo: {
    dica: 'Este trecho existe no que foi enviado ao Sculk ({{onde}}). Confira se ele sustenta a frase.',
    dicaJogador:
      'Este trecho é uma mensagem de chat de um jogador ({{onde}}). Existe no log, mas não prova nada sozinho.',
    dicaNaoVerificado: 'O Warden não achou um trecho que sustente esta frase.',
    conferidoRotulo: 'Evidência conferida:',
    jogadorRotulo: 'Mensagem de jogador:',
    naoVerificado: 'não verificado',
    idDesconhecido: 'não verificado: trecho que o Sculk não recebeu',
    jogador: 'baseado em mensagem de jogador',
    modsInexistentes: 'Citados pelo Sculk, mas não estão no pack: {{mods}}.',
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
    desempenho: 'desempenho do teste de {{sessao}}',
    comparacao: 'desempenho entre versões',
    perfilDesempenho: 'perfil de desempenho de {{sessao}}',
  },
  confianca: {
    rotulo: 'Confiança: {{nivel}}',
    low: 'baixa',
    medium: 'média',
    high: 'alta',
    como: 'Calculada pelo Warden a partir das evidências conferidas, não pelo que o Sculk diz de si.',
  },
  proposta: {
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
    titulo: 'O pack mudou desde o começo da conversa.',
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
  },
  composer: {
    rotulo: 'Pergunte mais ao Sculk',
    exemplo: 'Pergunte mais ou conte o que aconteceu',
    enviar: 'Enviar',
    dicaVazio: 'Escreva uma pergunta para enviar.',
    dicaLendo: 'Espere o Sculk terminar de ler.',
    depoisDoErro: 'Para perguntar outra coisa, a pergunta anterior precisa de uma resposta.',
  },
  detalhes: {
    botao: 'Detalhes',
    botaoAvisos_one: 'Detalhes, {{count}} aviso',
    botaoAvisos_other: 'Detalhes, {{count}} avisos',
    titulo: 'Detalhes',
    fechar: 'Fechar Detalhes',
    fecharDica: 'Fechar (Esc)',
    servico: 'Serviço e uso',
    provedor: 'Provedor',
    modelo: 'Modelo',
    modeloLeve_one: '{{count}} resposta usou o {{modelo}}',
    modeloLeve_other: '{{count}} respostas usaram o {{modelo}}',
    modo: 'Modo',
    modoGemini: 'Não se aplica ao Gemini',
    modoMisto: '{{rapido}} rápido, {{fundo}} a fundo',
    modoAntigo: 'Escolhido em Configurações, antes do modo por pergunta',
    tokens: 'Tokens',
    tokensSub: 'Contados pelo provedor.',
    pedidos: 'Pedidos',
    comecou: 'Começou',
    podeLer: 'O Sculk pode ler',
    leituras: {
      pack: 'Informações do pack e os achados do Warden',
      logs: 'Logs e crash reports',
      logsSub: 'Sem dados pessoais.',
      desempenho: 'Medidas de desempenho dos testes',
      configs: 'Configs do pack',
      github: 'Páginas dos mods no GitHub',
      githubSub: 'Só o nome do mod e palavras do erro.',
      negado: 'Não permitido nesta conversa.',
    },
    enviados: 'Tudo o que foi enviado',
    textoInicial: 'Texto inicial',
    total: 'Total',
    avisos: 'Avisos desta conversa',
    quota_one: 'O limite diário do modelo mais forte acabou; {{count}} resposta usou o mais leve ({{modelo}}).',
    quota_other:
      'O limite diário do modelo mais forte acabou; {{count}} respostas usaram o mais leve ({{modelo}}).',
    timeout_one:
      'O modelo mais forte demorou demais para responder; {{count}} resposta usou o mais leve ({{modelo}}).',
    timeout_other:
      'O modelo mais forte demorou demais para responder; {{count}} respostas usaram o mais leve ({{modelo}}).',
    planoGratuito:
      'No plano gratuito, o Google pode usar o conteúdo enviado para melhorar os produtos dele.',
    apagar: 'Apagar esta conversa',
  },
  tokens: {
    mil: '~{{valor}} mil tokens',
    unidades: '{{valor}} tokens',
    milCurto: '~{{valor}} mil',
  },
  pedir: {
    semCausa: 'Ou converse com o Sculk sobre este travamento.',
    comCausa: 'Não parece ser isso?',
    texto:
      'Ele consulta o log, o pack e os testes. Antes de começar, você vê o que ele poderá consultar; nada é enviado sem a sua confirmação.',
    botao: 'Perguntar ao Sculk',
  },
  travamentos: {
    conversar: 'Conversar com o Sculk',
    conversarRotulo: 'Conversar com o Sculk sobre o travamento de {{quando}}',
  },
  configuracoes: {
    titulo: 'Sensor Sculk',
    texto:
      'O Sensor Sculk usa um destes provedores, com a sua chave. As conversas novas usam o provedor escolhido; cada conversa continua no provedor em que começou.',
    provedor: 'Provedor do Sculk',
    trocado: 'Conversas novas vão usar {{provedor}}.',
    chaves: 'Chave de cada provedor',
    modeloTitulo: 'Modelos ({{provedor}})',
    modos:
      'Procurar rápido usa o {{rapido}}; Procurar a fundo usa o {{fundo}}. Você escolhe a cada pergunta, no Sensor Sculk.',
    modosSemNome:
      'Você escolhe Procurar rápido ou Procurar a fundo a cada pergunta, no Sensor Sculk.',
    modelo: 'Hoje: {{modelo}}.',
    gemini: {
      titulo: 'Usa sempre o modelo mais forte do plano gratuito',
      desc: 'Se o limite diário do plano gratuito acabar, o Warden troca sozinho para um modelo mais leve e avisa.',
      descLeve:
        'Se o limite diário do plano gratuito acabar, o Warden troca sozinho para o mais leve ({{modelo}}), só naquela pergunta, e avisa.',
    },
    semChave: {
      gemini: 'O nome do modelo aparece depois de salvar a chave do Gemini.',
      openai: 'Os nomes dos modelos aparecem depois de salvar a chave da OpenAI.',
      anthropic: 'Os nomes dos modelos aparecem depois de salvar a chave do Claude.',
    },
    custo: {
      openai:
        'A OpenAI cobra por uso, na sua conta, conforme os preços dela. Não há plano gratuito pela API.',
      anthropic:
        'A Anthropic cobra por uso, na sua conta, conforme os preços dela. Não há plano gratuito pela API.',
    },
    usoChave: 'Usada só no Sensor Sculk, e sempre com a sua confirmação antes de cada envio.',
  },
  erros: {
    KEY_INVALID: {
      gemini:
        'O Google recusou a chave do Gemini. Confira a chave em Configurações → Chaves e contas.',
      openai: 'A OpenAI recusou a chave. Confira a chave em Configurações → Chaves e contas.',
      anthropic:
        'A Anthropic recusou a chave do Claude. Confira a chave em Configurações → Chaves e contas.',
    },
    RATE_LIMITED: {
      gemini:
        'O limite por minuto do plano gratuito continuou mesmo depois de esperar. Espere um pouco e tente de novo.',
      openai:
        'O limite por minuto da sua chave da OpenAI continuou mesmo depois de esperar. Espere um pouco e tente de novo.',
      anthropic:
        'O limite por minuto da sua chave da Anthropic continuou mesmo depois de esperar. Espere um pouco e tente de novo.',
    },
    QUOTA_EXCEEDED: {
      gemini:
        'O limite diário do plano gratuito do Gemini acabou, também no modelo mais leve. Tente de novo mais tarde: o limite volta a cada dia.',
      openai:
        'A cota da sua conta da OpenAI acabou ou chegou ao limite de gastos. Confira o "Billing" da OpenAI ou tente de novo mais tarde.',
      anthropic:
        'A cota da sua conta da Anthropic acabou ou chegou ao limite de gastos. Confira o "Billing" no Console da Anthropic ou tente de novo mais tarde.',
    },
    TIMEOUT: {
      gemini:
        'O Google não respondeu a tempo, nem nas novas tentativas. Confira a internet e tente de novo.',
      openai:
        'A OpenAI não respondeu a tempo, nem nas novas tentativas. Confira a internet e tente de novo.',
      anthropic:
        'A Anthropic não respondeu a tempo, nem nas novas tentativas. Confira a internet e tente de novo.',
    },
    BILLING: {
      gemini:
        'O Google pediu cobrança para esta chave (faturamento desligado ou sem crédito). Confira o plano da sua chave no Google AI Studio.',
      openai:
        'A OpenAI pediu pagamento para esta chave (sem crédito ou sem forma de pagamento). Confira o "Billing" da sua conta da OpenAI.',
      anthropic:
        'A Anthropic pediu pagamento para esta chave (sem crédito). Confira o "Billing" no Console da Anthropic.',
    },
    UNAVAILABLE: {
      gemini:
        'O Google está sobrecarregado agora e não respondeu nas novas tentativas. Tente de novo daqui a pouco.',
      openai:
        'A OpenAI está sobrecarregada agora e não respondeu nas novas tentativas. Tente de novo daqui a pouco.',
      anthropic:
        'A Anthropic está sobrecarregada agora e não respondeu nas novas tentativas. Tente de novo daqui a pouco.',
    },
    NO_MODEL: {
      gemini:
        'Nenhum modelo do Gemini disponível para a sua chave serve para o Sculk. Confira a chave em Configurações.',
      openai:
        'Nenhum modelo da OpenAI disponível para a sua chave serve para o Sculk. Confira a chave em Configurações.',
      anthropic:
        'Nenhum modelo do Claude disponível para a sua chave serve para o Sculk. Confira a chave em Configurações.',
    },
    REJECTED: {
      gemini:
        'O Google recusou o pedido (código {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
      openai:
        'A OpenAI recusou o pedido (código {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
      anthropic:
        'A Anthropic recusou o pedido (código {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    ia: typeof ia;
  }
}
