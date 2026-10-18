/**
 * Busca do culpado (SPEC T25; ROADMAP D-12): o diálogo "Encontrar o mod culpado", o modo da
 * tela do teste (trilha de rodadas, "O que o Warden está fazendo agora", tabela, suspeitos), o
 * modo assistido, o resultado e as entradas (menu ▾, resultado do travamento, Travamentos).
 *
 * Glossário (QUALITY §8): "encontrar o mod culpado", "busca do culpado", "rodada"; nunca
 * "bisseção" nem "bisect".
 */
export const buscaDoCulpado = {
  botao: 'Buscando o culpado: ver progresso',
  etapa: {
    preparar: 'Preparando a busca do culpado',
    mundo: 'Criando o mundo de teste',
    rodada: 'Rodada da busca do culpado',
  },
  menu: {
    item: 'Encontrar o mod culpado…',
    descricao: 'Abre o jogo várias vezes com partes do pack até achar o mod',
  },
  dialogo: {
    titulo: 'Encontrar o mod culpado',
    sub: 'O Warden abre o jogo várias vezes com partes do pack, numa cópia separada. Seu pack, a instância de teste e seus mundos não mudam.',
    carregando: 'Lendo o pack…',
    oQue: 'O que acontece no jogo?',
    estimativa: 'Estimativa',
    estimativaSemTeste:
      'O Warden ainda não sabe quanto este pack leva para entrar no mundo; a estimativa fica mais certa depois da primeira rodada.',
    entram_one: '{{count}} mod entra na busca.',
    entram_other: '{{count}} mods entram na busca.',
    sempre_one: '{{sempre}} fica sempre ligado.',
    sempre_other: '{{sempre}} ficam sempre ligados.',
    bibliotecas:
      'As bibliotecas exigidas sempre vão junto com quem precisa delas, para não aparecer um erro diferente.',
    desligados: '{{nomes}} ficam desligados durante a busca, para não atrapalhar.',
    semDesligados: 'Nenhuma ferramenta do jogador para desligar durante a busca.',
    mundo: 'Cada rodada entra numa cópia do “Mundo de teste”, apagada no fim.',
    criarMundo:
      'Antes da primeira rodada, o Warden cria o “Mundo de teste” com o servidor do Minecraft {{minecraft}}, sem mods e sempre igual. Só desta vez; leva alguns minutos.',
    comoFunciona:
      'Se a rodada travar igual, o culpado está entre os mods ligados; se não travar, está no resto. A cada rodada sobra metade dos suspeitos. Dá para pausar entre rodadas e cancelar quando quiser; ao cancelar, o Warden mostra até onde chegou.',
    naoMexa:
      'Não mexa na janela do jogo durante a busca: o Warden entra no mundo e fecha o jogo sozinho. Enquanto a busca roda, não dá para testar outra coisa.',
    pausadaTitulo: 'Há uma busca pausada neste pack',
    pausadaTexto: 'Começar uma busca nova descarta a pausada.',
    continuarPausada: 'Continuar a busca pausada',
    semMods: 'Este pack não tem mods para procurar.',
    cancelar: 'Cancelar',
    comecar: 'Começar a busca',
  },
  sintoma: {
    abrir: {
      titulo: 'Trava ao abrir o jogo',
      desc: 'Fecha antes do menu principal.',
    },
    mundo: {
      titulo: 'Trava ao entrar no mundo',
      desc: 'Abre o menu, mas trava quando entra no mundo.',
      semEula: 'Precisa do mundo de teste: aceite a EULA abaixo para o Warden criá-lo.',
    },
    linha: {
      titulo: 'Aparece uma mensagem no log',
      desc: 'O Warden procura a mensagem em cada rodada.',
      campo: 'Mensagem do log',
      ajuda: 'Copie do console um trecho que só aparece com o problema.',
    },
    assistido: {
      titulo: 'Outra coisa: eu digo se aconteceu',
      desc: 'A cada rodada você joga e responde. Para problemas que precisam de uma ação no jogo.',
    },
  },
  estimativa: {
    rodadas: '{{min}} a {{max}} rodadas',
    rodadasUma_one: '{{count}} rodada',
    rodadasUma_other: '{{count}} rodadas',
    minutos: 'cerca de {{min}} a {{max}} minutos',
    minutosUm_one: 'cerca de {{count}} minuto',
    minutosUm_other: 'cerca de {{count}} minutos',
  },
  minutos_one: '{{count}} min',
  minutos_other: '{{count}} min',
  tela: {
    titulo: 'Buscando o mod culpado',
    carregando: 'Lendo a busca do culpado…',
    vazioTitulo: 'Nenhuma busca do culpado neste pack',
    vazioTexto:
      'Quando um problema se repete e o Warden não acha a causa no log, a busca abre o jogo com partes do pack até achar o mod.',
    encontrar: 'Encontrar o mod culpado…',
    rodadas: 'Rodadas',
    lado: 'Suspeitos e mods fixos da busca',
  },
  linha: {
    andamento_one:
      'Rodada {{rodada}} de {{min}} a {{max}} · {{count}} mod ainda suspeito · {{tempo}} até agora',
    andamento_other:
      'Rodada {{rodada}} de {{min}} a {{max}} · {{count}} mods ainda suspeitos · {{tempo}} até agora',
    pausada_one:
      'Pausada antes da rodada {{rodada}} · {{count}} mod ainda suspeito · {{tempo}} até agora',
    pausada_other:
      'Pausada antes da rodada {{rodada}} · {{count}} mods ainda suspeitos · {{tempo}} até agora',
  },
  acoes: {
    pausar: 'Pausar depois desta rodada',
    pausando: 'Pausa pedida',
    continuar: 'Continuar a busca',
    cancelar: 'Cancelar busca',
  },
  agora: {
    titulo: 'Agora: rodada {{rodada}}',
    preparando: 'Preparando a cópia do pack para a próxima rodada…',
    mundo:
      'Criando o mundo de teste com o servidor do Minecraft, sem mods. Só na primeira busca deste pack; leva alguns minutos.',
    inteiro:
      'Rodada {{rodada}}: abrindo o jogo com o pack inteiro (tentativa {{tentativa}}) para ver se o problema se repete.',
    controle: 'Rodada {{rodada}}: abrindo o jogo sem mods, para ver se o problema é de um mod.',
    prefixo: 'Rodada {{rodada}}: abrindo o jogo com {{ligados}} dos {{total}} mods.',
    acrescentados_one: 'Se travar igual, o culpado é o mod acrescentado agora.',
    acrescentados_other:
      'Se travar igual, o culpado está entre os {{count}} mods acrescentados agora.',
    sozinho: 'Rodada {{rodada}}: abrindo o jogo só com {{nomes}}, para ver se basta para travar.',
    sem: 'Rodada {{rodada}}: abrindo o jogo com o pack inteiro sem {{nomes}}, para confirmar.',
    espera: 'Espera entrar no mundo e ficar 20 s sem travar.',
    esperaAssistido: 'Quando o jogo entrar, o Warden pergunta o que aconteceu.',
    pausaPedida: 'A busca pausa quando esta rodada terminar.',
  },
  eula: {
    titulo: 'Para entrar no mundo, o Warden cria um mundo de teste',
    texto:
      'Os seus mundos foram salvos com todos os mods e não servem para as rodadas, que desligam parte deles. Na primeira busca deste pack, o Warden abre por alguns minutos o servidor do Minecraft, sem mods e só neste computador, para criar um mundo novo. Para abrir o servidor, a Mojang exige que você aceite a EULA, a licença de uso do jogo.',
    semAceite:
      'O Warden lembra a resposta, a mesma do “Testar como servidor” (dá para mudar em Configurações → Teste). Sem o aceite, “Trava ao entrar no mundo” fica indisponível e as rodadas vão só até o menu principal.',
    ler: 'Ler a EULA (aka.ms/MinecraftEULA)',
    aceitar: 'Aceito a EULA',
    aceitando: 'Aceitando…',
  },
  assistido: {
    titulo: 'Rodada {{rodada}} · modo assistido',
    texto_one:
      'O jogo está aberto com {{count}} mod. Faça no jogo o que causa o problema e diga o que aconteceu.',
    texto_other:
      'O jogo está aberto com {{count}} mods. Faça no jogo o que causa o problema e diga o que aconteceu.',
    aconteceu: 'Aconteceu',
    naoAconteceu: 'Não aconteceu',
    naoDeu: 'Não deu para testar',
    sozinho: 'Se o jogo travar, a rodada é marcada sozinha.',
  },
  pausada: {
    titulo: 'Busca pausada.',
    texto: 'O estado fica guardado: dá para continuar agora ou outro dia. Nenhum jogo está aberto.',
  },
  inferida: {
    titulo: 'O mod {{de}} usa {{para}} sem declarar a dependência',
    texto:
      'A busca refez a rodada com os dois juntos e passou a ligar {{para}} sempre que {{de}} entra.',
  },
  trilha: {
    rotulo: 'Rodadas da busca',
    legenda: 'Legenda das rodadas',
    rodada: 'Rodada {{rodada}}: {{resultado}}',
    rodadaMods_one: 'Rodada {{rodada}}, {{count}} mod: {{resultado}}',
    rodadaMods_other: 'Rodada {{rodada}}, {{count}} mods: {{resultado}}',
  },
  marca: {
    same: 'Travou igual',
    pass: 'Passou',
    diff: 'Travou diferente (não conta)',
    timeout: 'Tempo esgotado',
    now: 'Em andamento',
    ask: 'Esperando a sua resposta',
    paused: 'Pausada antes desta rodada',
    todo: 'Ainda não',
  },
  veredito: {
    passou: 'Passou',
    travouIgual: 'Travou igual',
    travouDiferente: 'Travou diferente (não conta)',
    tempo: 'Tempo esgotado (não conta)',
    ambiente: 'Problema do Java ou do computador',
    inferida: 'Faltou uma dependência; refeita',
    aconteceu: 'Aconteceu',
    naoAconteceu: 'Não aconteceu',
    naoDeu: 'Não deu para testar (não conta)',
  },
  rodada: {
    inteiro: 'Pack inteiro',
    controle: 'Sem mods',
    prefixo_one: 'O primeiro mod',
    prefixo_other: 'Os primeiros {{count}} mods',
    prefixoCom_one: 'O primeiro mod, com {{nomes}}',
    prefixoCom_other: 'Os primeiros {{count}} mods, com {{nomes}}',
    sozinho: 'Só {{nomes}}',
    sem: 'Pack sem {{nomes}}',
  },
  tabela: {
    legenda: 'Rodadas da busca do culpado',
    rodada: 'Rodada',
    ligados: 'Mods ligados',
    resultado: 'Resultado',
    tempo: 'Tempo',
    log: 'Log',
    mods: '({{n}})',
    repeticao: '· repetição {{vez}}',
    verLog: 'Ver o log da rodada {{rodada}}',
  },
  log: {
    titulo: 'Log da rodada {{rodada}}',
    carregando: 'Lendo o log…',
  },
  suspeitos: {
    faixa_one: '{{count}} de {{total}} mods ainda suspeito',
    faixa_other: '{{count}} de {{total}} mods ainda suspeitos',
    ver_one: 'Ver o mod suspeito',
    ver_other: 'Ver os {{count}} mods suspeitos',
  },
  lado: {
    sempre: 'Sempre ligados',
    soLoader: 'Só o loader',
    desligados: 'Desligados na busca',
    nenhum: 'Nenhum',
    explicacao:
      'O que o Warden faz: abre o jogo com uma parte dos mods. Se travar igual, o culpado está nessa parte; se não travar, está no resto. A cada rodada sobra metade dos suspeitos.',
  },
  cancelar: {
    titulo: 'Cancelar a busca do culpado?',
    texto:
      'O Warden fecha o jogo, apaga a cópia do pack da busca e guarda até onde chegou. Seu pack, a instância de teste e seus mundos não mudam.',
    confirmar: 'Cancelar busca',
    cancelando: 'Cancelando…',
    voltar: 'Continuar buscando',
  },
  resultado: {
    sub_one: 'Busca de {{quando}} · {{count}} rodada · {{tempo}}',
    sub_other: 'Busca de {{quando}} · {{count}} rodadas · {{tempo}}',
    secao: 'Resultado',
    deNovo: 'Começar de novo',
    testar: 'Testar de novo',
    verRodadas_one: 'Ver a rodada',
    verRodadas_other: 'Ver as {{count}} rodadas',
    confianca: 'Confiança',
    alta: 'Alta: confirmado em 2 testes de controle',
    media_one: 'Média: o problema não aparece sempre; cada “passou” foi conferido {{count}} vez',
    media_other:
      'Média: o problema não aparece sempre; cada “passou” foi conferido {{count}} vezes',
    tambemLigado: 'Também ligado',
    altera: 'Ver o que o {{nome}} altera no jogo',
    remover: 'Remover {{nome}} do pack',
    ajustes: 'Ajustes do teste',
    ligado: 'O resultado fica em Problemas → Travamentos, junto com o travamento de origem.',
    antes: 'Antes, a busca achou {{nomes}}.',
    entre_one: 'O culpado é este mod.',
    entre_other: 'O culpado está entre estes {{count}} mods.',
    achou: {
      titulo: 'Culpado encontrado: {{nome}}',
      tituloJunto: 'Culpado encontrado: {{nome}} (junto com {{outros}})',
      explicacao:
        'Sem o {{nome}}, o jogo funcionou normalmente; só com o {{nome}} (e o que ele exige), o problema apareceu igual.',
      explicacaoJunto:
        'Sem o {{nome}}, o jogo funcionou normalmente; só com o {{nome}} e {{outros}}, o problema apareceu igual.',
    },
    outro: {
      titulo: 'Há outro problema além deste.',
      texto:
        'Sem o {{nome}}, o pack ainda trava igual: outro mod também causa o problema. A busca pode continuar sem ele.',
      continuar: 'Continuar a busca sem o {{nome}}',
    },
    naoReproduziu: {
      titulo: 'O problema não se repetiu',
      texto:
        'O problema não apareceu em 3 testes seguidos. A busca só funciona com problemas que se repetem.',
    },
    semMods: {
      titulo: 'Mesmo sem mods o jogo trava',
      texto:
        'Mesmo sem mods o jogo trava. A causa está no Java, no driver de vídeo ou nos ajustes do teste.',
    },
    ambiente: {
      titulo: 'O problema não é de um mod',
      texto:
        'O jogo não abre com este Java: a busca parou, porque nenhuma rodada chegaria ao mundo. Confira o Java nos ajustes do teste.',
    },
    contradicao: {
      titulo: 'Os testes se contradizem',
      texto_one:
        'Os testes se contradizem: o mod {{nomes}} parece evitar o problema. Isso acontece com mods de compatibilidade.',
      texto_other:
        'Os testes se contradizem: os mods {{nomes}} parecem evitar o problema. Isso acontece com mods de compatibilidade.',
      semNomes:
        'Os testes se contradizem: o mesmo grupo de mods travou numa rodada e funcionou em outra. O problema pode não se repetir sempre, ou um mod evita o problema.',
    },
    inconclusiva: {
      titulo: 'A busca não chegou a uma resposta',
      texto:
        'Três rodadas seguidas travaram de outro jeito, passaram do tempo ou não deu para testar, e o Warden parou.',
    },
    cancelada: {
      titulo: 'Busca cancelada',
      nenhum: 'A busca parou antes de descartar algum mod.',
      texto_one: 'A busca parou antes do fim, mas já descartou {{count}} mod.',
      texto_other: 'A busca parou antes do fim, mas já descartou {{count}} mods.',
    },
  },
  oferta: {
    titulo: 'Encontrar o mod culpado',
    texto:
      'O Warden pode abrir o jogo em rodadas, com partes do pack, até achar o mod. Pode levar de 20 a 40 minutos num pack grande; dá para pausar e cancelar.',
    textoVezes:
      'Este travamento já aconteceu {{vezes}} vezes. O Warden pode abrir o jogo em rodadas, com partes do pack, até achar o mod. Pode levar de 20 a 40 minutos num pack grande; dá para pausar e cancelar.',
    botao: 'Encontrar o mod culpado…',
    igual: 'Igual ao travamento de {{quando}}.',
  },
  travamentos: {
    encontrar: 'Encontrar o mod culpado',
    rotulo: 'Encontrar o mod culpado do travamento de {{quando}}',
    verBusca: 'Ver a busca do culpado',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    buscaDoCulpado: typeof buscaDoCulpado;
  }
}
