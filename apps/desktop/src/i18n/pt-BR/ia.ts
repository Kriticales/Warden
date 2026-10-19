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
    sub: 'Converse com a IA ({{provedor}}) sobre um problema do pack. Ela consulta o pack, os logs e os testes; você vê tudo o que é enviado, e nada muda no pack sem a sua decisão.',
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
      'Para usar a IA, salve a chave do provedor escolhido em Configurações → Chaves e contas. A chave é sua: o uso é cobrado ou limitado nela, conforme o plano.',
    provedores:
      'O Warden funciona com o Gemini (Google), a OpenAI e o Claude (Anthropic). Para começar, o sugerido é o Gemini, que tem plano gratuito.',
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
    atual: 'Provedor: {{provedor}}. Modelo: {{modelo}} ({{nivel}}).',
    atualGemini: 'Provedor: Gemini. Modelo: {{modelo}}.',
    consultando: 'Consultando os modelos da sua chave…',
    nenhum: 'Nenhum modelo do provedor disponível para a sua chave.',
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
    paraValor: '{{provedor}} ({{modelo}}), com a sua chave',
    tamanho: 'Tamanho inicial',
    tamanhoValor:
      '{{bytes}} de texto; cerca de {{tokens}} com as instruções do Warden (token é a unidade que os provedores de IA usam para medir e cobrar)',
    tamanhoSoBytes: '{{bytes}} de texto',
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
      'A conversa e tudo o que foi enviado nela saem deste computador. O que já foi enviado à IA não volta.',
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
    sub: 'Conversa sobre {{origem}} · começou {{quando}} com {{provedor}}',
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
  },
  troca: {
    texto: 'O limite diário do modelo mais forte acabou; esta resposta usou o mais leve.',
    modelo: 'Modelo desta resposta: {{modelo}}.',
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
    provedor: 'Provedor',
    modelo: 'Modelo',
    tamanho: 'Esta conversa',
    inicial: 'Pedido inicial',
    inicialValor: '{{tokens}}, contados pelo provedor',
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
    titulo: 'IA',
    texto:
      'O Diagnóstico com IA usa um destes provedores, com a sua chave. As conversas novas usam o provedor escolhido; cada conversa continua no provedor em que começou.',
    provedor: 'Provedor da IA',
    trocado: 'Conversas novas vão usar {{provedor}}.',
    chaves: 'Chave de cada provedor',
    modeloTitulo: 'Modelo ({{provedor}})',
    economical: {
      titulo: 'Mais econômico (padrão)',
      desc: 'Para análises simples: o modelo mais barato da família mais nova.',
    },
    careful: {
      titulo: 'Análise mais cuidadosa',
      desc: 'Para problemas difíceis: o modelo mais capaz da família mais nova, que custa mais por conversa.',
    },
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
    usoChave: 'Usada só no Diagnóstico com IA, e sempre com a sua confirmação antes de cada envio.',
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
        'O limite diário do plano gratuito deste modelo acabou. Troque de modelo em Configurações (cada modelo tem a sua cota) ou tente de novo mais tarde.',
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
        'Nenhum modelo do Gemini disponível para a sua chave serve para o diagnóstico. Confira a chave em Configurações.',
      openai:
        'Nenhum modelo da OpenAI disponível para a sua chave serve para o diagnóstico. Confira a chave em Configurações.',
      anthropic:
        'Nenhum modelo do Claude disponível para a sua chave serve para o diagnóstico. Confira a chave em Configurações.',
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
