/**
 * Testar com perfil de desempenho (SPEC T13; ARCHITECTURE §7.7; ROADMAP L-12): o item do menu ▾
 * do Testar, a opção do "Testar como servidor" e a parte "Desempenho" do resultado (o tempo de
 * carregamento por mod e o resumo dos perfis do spark).
 */
export const perfilDesempenho = {
  menu: {
    item: 'Testar com perfil de desempenho',
    descricao: 'Mede o que mais pesa para carregar e rodar',
  },
  servidor: {
    opcao: 'Medir o desempenho do servidor com o spark',
    opcaoDesc:
      'Quando o jogo abre, o Warden liga o spark do servidor pelo console por 1 minuto e desliga no fim. Precisa do spark no pack.',
    soComJogo: 'Só com o jogo aberto junto.',
  },
  resultado: {
    titulo: 'Desempenho',
    subJogo: 'Teste com perfil de desempenho',
    subServidor: 'Teste como servidor com o perfil de desempenho',
    carregando: 'Lendo o desempenho…',
    carregamento: {
      titulo: 'O que mais demorou para carregar',
      tabela: 'Tempo de carregamento de cada mod',
      mod: 'Mod',
      tempo: 'Tempo',
      ressalva:
        'Mede o código de cada mod nas fases do {{loader}}. Não entram as texturas, os modelos e o que os mods alteram no jogo (mixins), que costumam ser a maior parte do tempo em packs grandes.',
      jogoELoader: 'O Minecraft e o {{loader}} levaram {{tempo}} nessas fases.',
      maisMods_one: 'E mais {{count}} mod.',
      maisMods_other: 'E mais {{count}} mods.',
      semDadoFabric: 'O Fabric não registra o tempo de carregamento de cada mod.',
      semDadoQuilt: 'O Quilt não registra o tempo de carregamento de cada mod.',
      semDadoVanilla: 'Sem mods para medir: o pack não tem loader.',
      semLog:
        'O log do {{loader}} não foi encontrado na instância de teste. O jogo fechou antes de carregar os mods?',
      semLinhas:
        'O log do {{loader}} ({{log}}) não trouxe o tempo de cada mod. O jogo fechou antes de terminar de carregar?',
      servidor: 'No servidor, o Warden mede só com o spark.',
    },
    spark: {
      tituloPerfil: 'Perfil do spark salvo às {{hora}}',
      tituloMemoria: 'Memória do spark salva às {{hora}}',
      tituloPerfilServidor: 'Perfil do spark do servidor, salvo às {{hora}}',
      tituloMemoriaServidor: 'Memória do spark do servidor, salva às {{hora}}',
      duracao: 'Duração',
      tick: 'Tempo por tick',
      tickValor: '{{mediana}} na mediana, {{pior}} no pior tick',
      tps: 'Ticks por segundo',
      tpsValor: '{{tps}} em média (o normal é 20)',
      processamento: 'Mais processamento',
      memoria: 'Mais memória',
      parte: '{{nome}} ({{porcentagem}})',
      soJogo: 'Nenhum mod se destacou: quase tudo foi do jogo, do loader e do Java.',
      jogo: 'O jogo, o loader e o Java: {{porcentagem}}.',
      ilegivel: 'Não deu para ler este arquivo do spark.',
      abrir: 'Abrir no visualizador do spark',
      abrirNota:
        'Abre o visualizador do spark no navegador e mostra o arquivo na pasta: arraste o arquivo para a página. Nada é enviado: o spark lê o arquivo aí mesmo.',
      abrindo: 'Abrindo…',
      nenhum: 'Nenhum perfil do spark salvo neste teste.',
      comoSalvar:
        'No jogo, digite /sparkc profiler start e, depois de jogar um pouco, /sparkc profiler stop --save-to-file. O Warden lê o arquivo no fim do teste.',
      semSparkServidor:
        'O spark não está entre os mods do servidor. Adicione o spark ao pack para medir o servidor.',
      falhouServidor:
        'O Warden não conseguiu ligar o spark do servidor (o servidor fechou antes?).',
      ligadoServidor:
        'O Warden ligou o spark do servidor quando o jogo abriu, mas nenhum perfil foi salvo.',
      deFora_one: '{{count}} arquivo mais antigo do spark ficou de fora.',
      deFora_other: '{{count}} arquivos mais antigos do spark ficaram de fora.',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    perfilDesempenho: typeof perfilDesempenho;
  }
}
