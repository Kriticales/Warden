/**
 * Textos do diagnóstico sem IA (SPEC T13 passos 1, 4 e 7; T14; ROADMAP D-03): seção Problemas,
 * "Por que travou" no resultado do teste, o diálogo de erros do Testar e as marcas da lista de
 * Mods. Linguagem simples para quem não programa: o que aconteceu, qual mod, o que fazer; o
 * detalhe técnico fica em "Como sabemos".
 *
 * Parâmetros das frases dos achados (montados em `features/diagnostics/text.ts`):
 * - `nome`: o item principal (nome no pack, ou o id/arquivo que o log cita);
 * - `outro`: o segundo item (pares: duplicados, conflitos);
 * - `dep`: a dependência que falta (nome conhecido ou o id);
 * - `modNome`, `depNome`: nome do mod e da dependência como o log escreve (com o id quando o
 *   log não traz o nome);
 * - os demais vêm direto do padrão do log (`range`, `actual`, `java_required`, `argument`…).
 *
 * `regras.<CÓDIGO>` são os achados do pré-teste (ARCHITECTURE §9.2) e `achados.<chave>` os da
 * análise pós-crash (`crates/warden-diagnostics/data/log-patterns.toml`, `title_key`).
 */
export const diagnostico = {
  etapa: {
    verificar: 'Verificando o pack',
    final: 'Verificação final',
  },
  etapas: {
    verificar: 'Verificar o pack',
    final: 'Verificação final',
  },
  secao: {
    titulo: 'Problemas',
    nunca: 'O pack ainda não foi verificado neste computador.',
    verificando: 'Verificando…',
    resumo_one: '{{count}} encontrado · última verificação {{quando}}',
    resumo_other: '{{count}} encontrados · última verificação {{quando}}',
    resumoVazio: 'Última verificação {{quando}}',
    agora: 'agora há pouco',
    tambem: 'A verificação também roda sozinha ao clicar em Testar.',
    rapida:
      'Alguns mods ainda não foram baixados para o teste, então os arquivos deles não foram lidos. A verificação completa roda ao clicar em Testar.',
    verificarAgora: 'Verificar agora',
    verificandoMeta:
      'Dependências, versões, Java, duplicatas e conflitos conhecidos, com os dados do pack, das fontes e dos arquivos dos mods.',
    erroVerificar: 'Não deu para verificar o pack agora.',
    vazioTitulo: 'Nenhum problema encontrado',
    vazioTexto_one:
      'Dependências, versões, Java, duplicatas e conflitos conhecidos foram conferidos no item do pack.',
    vazioTexto_other:
      'Dependências, versões, Java, duplicatas e conflitos conhecidos foram conferidos nos {{count}} itens do pack.',
    semLoaderTitulo: 'Nada para conferir neste pack',
    semLoaderTexto:
      'O pack não usa Forge, NeoForge, Fabric nem Quilt, então não há dependências de mods para conferir.',
    contagem_one: '{{count}} problema',
    contagem_other: '{{count}} problemas',
    ignorados_one: '{{count}} aviso ignorado por você neste pack',
    ignorados_other: '{{count}} avisos ignorados por você neste pack',
    mostrarIgnorados: 'Mostrar os ignorados',
    esconderIgnorados: 'Esconder os ignorados',
  },
  grupos: {
    error: 'Erros',
    errorNota: 'impedem o teste, a menos que você escolha testar mesmo assim',
    warning: 'Avisos',
    warningNota: 'não impedem o teste',
    info: 'Informações',
    infoNota: 'só para você saber',
  },
  gravidade: {
    error: 'Erro',
    warning: 'Aviso',
    info: 'Informação',
  },
  achado: {
    comoSabemos: 'Como sabemos',
    itens: 'Itens envolvidos',
    ignorar: 'Ignorar neste pack',
    desfazerIgnorar: 'Voltar a mostrar',
    ignorado: 'ignorado por você',
    ignoradoToast: 'Este aviso fica ignorado neste pack.',
    voltouToast: 'O aviso voltou a valer neste pack.',
    verItem: 'Ver {{nome}}',
    foraDoPack: 'não está no pack',
  },
  evidencia: {
    jar: 'O arquivo {{arquivo}} de {{item}} diz:',
    api: '{{fonte}} informa (campo {{campo}}).',
    log: '{{arquivo}}, linha {{linha}}:',
    curada: 'Lista de conflitos conhecidos do Warden ({{regra}}):',
    arquivo: '{{caminho}}:',
    endereco: 'Consulta: {{url}}',
  },
  correcao: {
    adicionar: 'Adicionar {{nome}}',
    adicionarProcurando: 'Procurando {{nome}}…',
    adicionarNaoAchou:
      'O Warden não achou {{nome}} no Modrinth. Procure pelo nome em Adicionar e escolha a versão para o pack.',
    abrirAdicionar: 'Abrir Adicionar',
    remover: 'Remover {{nome}}',
    atualizar: 'Atualizar {{nome}}',
    lado: {
      client: 'Mudar lado para Só cliente',
      server: 'Mudar lado para Só servidor',
      both: 'Mudar lado para Cliente e servidor',
    },
    ladoFeito: 'Lado de {{nome}} mudado.',
    java: 'Mudar o Java do teste',
    javaVersao: 'Usar o Java {{major}} no teste',
    memoria: 'Aumentar a memória do teste',
    argumento: 'Tirar {{argumento}} dos argumentos',
    config: 'Abrir {{caminho}} em Configs',
    padrao: 'Abrir {{caminho}} em Configs',
  },
  ajuda: {
    corrigido: 'Pronto. O Warden verifica o pack de novo.',
  },
  /** Diálogo do Testar com erros (SPEC T13 passos 1 e 4; protótipo `teste-checagem`). */
  dialogo: {
    titulo_one: '{{count}} erro pode impedir o jogo de abrir',
    titulo_other: '{{count}} erros podem impedir o jogo de abrir',
    sub: 'Dá para corrigir agora, aqui mesmo, ou testar assim e ver o que acontece.',
    avisos_one:
      '{{count}} aviso também foi encontrado. Ele não impede o teste e fica em Problemas.',
    avisos_other:
      '{{count}} avisos também foram encontrados. Eles não impedem o teste e ficam em Problemas.',
    testarMesmoAssim: 'Testar mesmo assim',
    cancelar: 'Cancelar e ver Problemas',
    testarDeNovo: 'Testar de novo',
    carregando: 'Lendo os erros encontrados…',
    paradoCheck: 'O teste parou ao verificar o pack',
    paradoFinal: 'O teste parou na verificação final',
    paradoTexto_one: 'O Warden encontrou {{count}} erro que pode impedir o jogo de abrir.',
    paradoTexto_other: 'O Warden encontrou {{count}} erros que podem impedir o jogo de abrir.',
    verErros: 'Ver os erros',
  },
  /** "Por que travou" (SPEC T13 passo 7; protótipo `teste-travou`). */
  travou: {
    analisando: 'Procurando a causa nos logs…',
    semCausaTitulo: 'Não encontramos a causa automaticamente.',
    semCausaTexto:
      'O log não tem nenhum dos padrões que o Warden conhece (dependência faltando, mod duplicado, Java errado, falta de memória, entre outros). As últimas linhas com erro estão abaixo.',
    erroAnalise: 'Não deu para analisar este travamento agora.',
    outrasPistas_one: 'Mais {{count}} pista encontrada nos logs',
    outrasPistas_other: 'Mais {{count}} pistas encontradas nos logs',
    lidos: 'Arquivos lidos: {{arquivos}}.',
    copiar: 'Copiar relatório',
    copiado: 'Relatório copiado, sem dados pessoais.',
    copiarFalhou: 'Não deu para copiar o relatório.',
  },
  /** "Último travamento" na seção Problemas (até a D-06 trazer a lista de Travamentos). */
  ultimo: {
    titulo: 'Último travamento',
    quando: 'Teste de {{quando}} · versão {{versao}}',
    quandoSemVersao: 'Teste de {{quando}}',
    ver: 'Ver o que causou',
    nenhum: 'Nenhum teste deste pack travou.',
  },
  /** Relatório copiado ("Copiar relatório"), em texto simples. */
  relatorio: {
    cabecalho: 'Warden: por que o jogo travou',
    pack: 'Pack: {{nome}}',
    teste: 'Teste: {{quando}} ({{duracao}} de jogo)',
    jogo: 'Jogo: {{jogo}} · Java {{java}}',
    causa: 'Causa provável',
    semCausa: 'Causa não encontrada automaticamente.',
    evidencia: 'Evidência',
    itens: 'Mods envolvidos: {{nomes}}',
  },
  /** Marca na lista de Mods (SPEC T06). */
  marca: {
    error: 'Erro',
    warning: 'Aviso',
    dica_one: '{{count}} problema em Problemas',
    dica_other: '{{count}} problemas em Problemas',
  },
  fontes: {
    modrinth: 'O Modrinth',
    curseForge: 'A CurseForge',
  },
  regraGenerica: {
    titulo: 'Problema encontrado ({{regra}})',
    texto: 'Veja em "Como sabemos" o trecho que o Warden encontrou.',
  },
  regras: {
    E_LOADER: {
      titulo: '{{nome}} é para outro loader',
      texto:
        'O arquivo não tem os dados do loader deste pack, e o jogo não vai carregá-lo. Troque pela versão certa ou remova.',
    },
    E_MCVERSION: {
      titulo: '{{nome}} é de outra versão do Minecraft',
      texto: 'Use a versão feita para o Minecraft deste pack ou remova o mod.',
    },
    E_LOADERVERSION: {
      titulo: '{{nome}} pede outra versão do loader',
      texto: 'Troque a versão do mod ou a versão do loader do pack.',
    },
    E_MISSING_DEP: {
      titulo: '{{nome}} precisa de {{dep}}, que não está no pack',
      texto: 'Sem ele, o jogo costuma parar na tela de carregamento. Adicione a dependência.',
    },
    E_DEP_VERSION: {
      titulo: '{{nome}} precisa de outra versão de {{outro}}',
      texto: 'Atualize {{outro}} para uma versão que {{nome}} aceita.',
    },
    E_BREAKS: {
      titulo: '{{nome}} não funciona junto com {{outro}}',
      texto: 'O próprio {{nome}} avisa que quebra com {{outro}}. Mantenha só um dos dois.',
    },
    W_CONFLICTS: {
      titulo: '{{nome}} pode dar problema com {{outro}}',
      texto: 'O autor avisa que os dois juntos podem não funcionar bem.',
    },
    W_INCOMPATIBLE_API: {
      titulo: '{{nome}} é marcado como incompatível com {{outro}}',
      texto: 'A página do mod diz que eles não funcionam juntos.',
    },
    E_DUPLICATE_ID: {
      titulo: '{{nome}} está duas vezes no pack',
      texto: 'Dois arquivos trazem o mesmo mod, e o jogo não abre assim. Remova um deles.',
    },
    E_DUPLICATE_FILE: {
      titulo: '{{nome}} e {{outro}} são o mesmo mod',
      texto: 'O mesmo arquivo (ou o mesmo projeto, por duas fontes) está duas vezes. Remova um.',
    },
    E_EXCLUSIVE: {
      titulo: '{{nome}} e {{outro}} fazem a mesma coisa',
      texto: 'Os dois são da mesma categoria e travam o jogo quando estão juntos. Mantenha só um.',
    },
    E_KNOWN_CONFLICT: {
      titulo: '{{nome}} e {{outro}} não funcionam juntos',
      texto: 'É um conflito conhecido, mesmo que os mods não avisem. Mantenha só um dos dois.',
    },
    W_KNOWN_CONFLICT: {
      titulo: '{{nome}} e {{outro}} costumam dar problema juntos',
      texto: 'É um problema conhecido. Se o jogo travar, comece por eles.',
    },
    W_OBSOLETE: {
      titulo: '{{nome}} não é mais necessário nesta versão',
      texto: 'Ele foi substituído ou parou de ser mantido. Considere remover.',
    },
    W_SIDE: {
      titulo: '{{nome}} está no lado errado',
      texto:
        'Ele só funciona de um lado (cliente ou servidor), mas o pack o manda para os dois. Num servidor, isso pode impedir o servidor de abrir.',
    },
    E_JAVA: {
      titulo: '{{nome}} precisa de outro Java',
      texto: 'Mude o Java em Ajustes do teste, ou deixe no automático.',
    },
    E_JAVA_PACK: {
      titulo: 'O Java do teste não serve para esta versão do Minecraft',
      texto: 'Mude o Java em Ajustes do teste, ou deixe no automático.',
    },
    I_PRERELEASE: {
      titulo: '{{nome}} está numa versão de teste (alfa ou beta)',
      texto: 'Versões de teste podem ter problemas. Use a versão final quando ela sair.',
    },
    I_LEGACY_COREMOD: {
      titulo: '{{nome}} muda o jogo de um jeito antigo (coremod)',
      texto: 'É comum no Minecraft antigo. Se o jogo travar ao abrir, ele é um bom suspeito.',
    },
    I_PACK_FORMAT: {
      titulo: '{{nome}} foi feito para outra versão do Minecraft',
      texto:
        'O jogo pode mostrá-lo como incompatível, mas resource packs assim costumam funcionar.',
    },
    W_DOWNLOAD_BLOCKED: {
      titulo: '{{nome}} não baixa sozinho para os jogadores',
      texto:
        'O autor não deixa outros apps baixarem este arquivo. Quem jogar o pack vai precisar baixá-lo à mão.',
    },
  },
  achados: {
    crashReport: {
      titulo: 'O jogo travou: {{description}}',
      texto:
        'O Minecraft gravou um crash report com o erro {{exception}}. Ele diz em que ponto o jogo parou; os mods citados nele são os primeiros suspeitos.',
    },
    debugCrash: {
      titulo: 'Travamento pedido no jogo (F3 + C)',
      texto: 'Alguém segurou F3 + C, que fecha o jogo de propósito. Não é um problema do pack.',
    },
    tickingEntity: {
      titulo: 'Uma criatura ou objeto do mundo quebrou o jogo ({{entity_type}})',
      texto:
        'O erro aconteceu ao atualizar {{entity_type}}. O nome costuma apontar o mod dono. Atualize esse mod ou remova-o.',
    },
    tickingBlockEntity: {
      titulo: 'Um bloco do mundo quebrou o jogo ({{block_entity}})',
      texto:
        'O erro aconteceu ao atualizar o bloco {{block_entity}} (uma máquina, um baú…). O nome costuma apontar o mod dono.',
    },
    dependenciaFaltando: {
      titulo: '{{modNome}} precisa de {{depNome}}, que não está no pack',
      texto:
        'O loader parou de carregar porque falta a dependência {{depNome}}{{faixa}}. Adicione a dependência.',
    },
    dependenciaVersaoErrada: {
      titulo: '{{modNome}} precisa de outra versão de {{depNome}}',
      texto:
        'O pack tem {{depNome}} {{actual}}, mas {{modNome}} pede {{range}}. Atualize um dos dois.',
    },
    versaoDoMinecraft: {
      titulo: '{{modNome}} é de outra versão do Minecraft',
      texto: 'Ele pede o Minecraft {{range}}. Use a versão feita para este Minecraft ou remova.',
    },
    versaoDoLoader: {
      titulo: '{{modNome}} pede outra versão do loader',
      texto: 'Ele pede {{dependency}} {{range}}, e o pack tem {{actual}}.',
    },
    javaDoMod: {
      titulo: '{{modNome}} precisa de um Java mais novo',
      texto:
        'Ele pede o Java {{range}}, e o teste usou o {{actual}}. Mude o Java em Ajustes do teste.',
    },
    dependenciaDoOutroLado: {
      titulo: '{{modNome}} precisa de {{depNome}} deste lado',
      texto:
        'A dependência está no pack, mas marcada só para o outro lado (cliente ou servidor). Mude o lado dela.',
    },
    modDuplicado: {
      titulo: '{{nome}} está duas vezes no pack',
      texto: 'O mesmo mod está em mais de um arquivo, e o jogo não abre assim. Remova um deles.',
    },
    modsIncompativeis: {
      titulo: '{{modNome}} não funciona junto com {{other_name}}',
      texto: 'O próprio mod avisa que quebra com o outro. Mantenha só um dos dois.',
    },
    modsEmConflito: {
      titulo: '{{modNome}} pode dar problema com {{other_name}}',
      texto: 'O autor avisa que os dois juntos podem não funcionar bem.',
    },
    pacoteRepetido: {
      titulo: 'Dois arquivos trazem a mesma parte de código ({{package}})',
      texto:
        'Os arquivos {{module}} e {{other}} têm o mesmo pacote Java, e o Forge não abre assim. Costuma ser o mesmo mod duas vezes, ou uma biblioteca repetida.',
    },
    nomeDeJarInvalido: {
      titulo: 'O nome do arquivo {{file}} não é aceito pelo Forge',
      texto: 'Renomeie o arquivo (sem espaços nem caracteres especiais) ou baixe-o de novo.',
    },
    jarDeOutroLoader: {
      titulo: '{{file}} é para outro loader',
      texto:
        'O arquivo foi feito para {{other_loader}}. Troque pela versão deste loader ou remova.',
    },
    jarIgnoradoPeloLoader: {
      titulo: '{{file}} foi ignorado pelo loader',
      texto:
        'O arquivo é de outro loader. O jogo abre sem ele: troque pela versão certa ou remova.',
    },
    provedorDeLinguagem: {
      titulo: '{{file}} precisa de outra versão do loader',
      texto:
        'Ele pede {{provider}} {{needed}}, e o loader tem {{found}}. Atualize o loader ou troque a versão do mod.',
    },
    modQuebrouAoIniciar: {
      titulo: '{{modNome}} deu erro ao iniciar',
      texto:
        'O jogo parou quando ele começou a carregar. Atualize o mod; se continuar, tente sem ele.',
    },
    mixinFalhou: {
      titulo: 'Uma alteração do jogo feita por {{modNome}} falhou',
      texto:
        'O mod tentou mudar uma parte do jogo e não conseguiu (config {{mixin_config}}). Em geral ele é de outra versão do jogo ou de um mod que ele altera. Atualize-o ou remova.',
    },
    javaVelho: {
      titulo: 'O Java do teste é velho demais para um mod',
      texto:
        'Uma parte do jogo foi feita para um Java mais novo. Deixe o Java no automático em Ajustes do teste, ou atualize o mod.',
    },
    javaNovoDemais: {
      titulo: 'O Java do teste é novo demais para esta versão do Minecraft',
      texto:
        'Versões antigas do Minecraft precisam do Java 8. Deixe o Java no automático em Ajustes do teste.',
    },
    javaForge1165: {
      titulo: 'Este Forge 1.16.5 não abre com o Java instalado',
      texto: 'Atualize o Forge do pack ou deixe o Java no automático em Ajustes do teste.',
    },
    faltaDeMemoria: {
      titulo: 'O jogo ficou sem memória',
      texto: 'Aumente a memória do teste em Ajustes do teste (8 GB costuma bastar).',
    },
    memoriaDoComputador: {
      titulo: 'O computador ficou sem memória para o jogo',
      texto: 'Feche outros programas ou diminua a memória do teste em Ajustes do teste.',
    },
    memoriaNaoReservada: {
      titulo: 'O Java não conseguiu reservar a memória pedida ({{size}})',
      texto: 'Diminua a memória do teste em Ajustes do teste, ou deixe no automático.',
    },
    memoriaInvalida: {
      titulo: 'O valor de memória nos argumentos não é válido',
      texto: 'Confira os argumentos da JVM em Ajustes do teste ({{argument}}).',
    },
    argumentoDaJvm: {
      titulo: 'O Java não reconhece um argumento do teste',
      texto: 'Tire {{argument}} dos argumentos da JVM em Ajustes do teste.',
    },
    driverDeVideo: {
      titulo: 'O driver de vídeo não deixou o jogo abrir',
      texto: 'Atualize o driver da placa de vídeo pelo site do fabricante e tente de novo.',
    },
    quedaNativa: {
      titulo: 'O próprio Java caiu',
      texto:
        'Costuma ser falta de memória do computador, driver de vídeo ou um mod com código nativo. O relatório do Java diz onde caiu.',
    },
    configCorrompida: {
      titulo: 'Uma config não pôde ser lida ({{config}})',
      texto:
        'O arquivo está com erro de formato. Restaure a versão do pack ou apague o arquivo da instância de teste.',
    },
    classeFaltando: {
      titulo: 'Falta uma parte de código ({{class}})',
      texto:
        'Um mod precisa de outro que não está no pack, ou é de outra versão. A linha do log aponta qual.',
    },
    falhaAoEntrarNoMundo: {
      titulo: 'O jogo não conseguiu entrar no mundo',
      texto:
        'O erro {{exception}} aconteceu ao colocar o jogador no mundo. Os mods citados logo abaixo no log são os suspeitos.',
    },
    mundoComModAusente: {
      titulo: 'O mundo de teste foi salvo com um mod que não está mais no pack ({{mod}})',
      texto:
        'Isso pode apagar blocos e itens desse mod no mundo. Se não precisar dele, apague os mundos de teste.',
    },
    mundoComOutraVersao: {
      titulo: 'O mundo de teste foi salvo com outra versão de {{mod}}',
      texto:
        'Versões diferentes podem estragar o mundo. Se der problema, apague os mundos de teste.',
    },
    forgeEsperaConfirmacao: {
      titulo: 'O Forge parou esperando uma confirmação',
      texto:
        'O mundo usa mods que não estão mais no pack. Apague os mundos de teste ou devolva os mods.',
    },
    mundoDeVersaoMaisNova: {
      titulo: 'O mundo de teste é de uma versão mais nova do Minecraft',
      texto: 'Apague os mundos de teste para criar um mundo novo nesta versão.',
    },
    mundoFormatoAntigo: {
      titulo: 'O mundo de teste está num formato antigo',
      texto: 'Se o jogo não carregar o mundo, apague os mundos de teste.',
    },
    mundoSemConfiguracao: {
      titulo: 'As configurações do mundo sumiram',
      texto:
        'Costuma ser um pacote de dados ou um mod de geração de mundo. Teste sem os mods de geração mais novos.',
    },
    pacoteDeDadosInvalido: {
      titulo: 'Um pacote de dados não pôde ser lido ({{datapack}})',
      texto: 'Atualize ou remova o pacote de dados (ou o mod que o traz).',
    },
    serverPropertiesInvalido: {
      titulo: 'O arquivo server.properties tem um erro',
      texto:
        'Abra o arquivo e corrija o texto com \\u quebrado, ou apague-o para o jogo criar um novo.',
    },
    codigoDeConduta: {
      titulo: 'O servidor não achou a pasta do código de conduta',
      texto: 'Crie a pasta configurada ou tire a opção do server.properties.',
    },
    autenticacaoInacessivel: {
      titulo: 'Os servidores da Mojang não responderam',
      texto: 'No teste offline do Warden isso não atrapalha.',
    },
    carregamentoFalhou: {
      titulo: 'O carregamento dos mods falhou',
      texto: 'O loader mostrou a tela de erro. As outras pistas abaixo dizem qual mod.',
    },
    excecaoFatal: {
      titulo: 'Um erro derrubou o jogo ({{exception}})',
      texto: 'O erro aconteceu na parte "{{thread}}" do jogo. A linha do log ajuda a achar o mod.',
    },
    crashReportGravado: {
      titulo: 'O jogo avisou que travou',
      texto: 'Ele gravou um crash report ({{crash_report}}). Abra o arquivo para ver os detalhes.',
    },
    jogoParou: {
      titulo: 'O jogo parou de responder',
      texto:
        'O jogo ficou travado e foi fechado. Costuma ser um mod pesado demais ou falta de memória.',
    },
  },
};

declare module '../catalogo' {
  interface Catalogo {
    diagnostico: typeof diagnostico;
  }
}
