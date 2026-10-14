/**
 * Textos do servidor local e do "Testar como servidor" (SPEC T27 e T21; ROADMAP L-09; decisões
 * D18 e D19). Termos do glossário (QUALITY §8.2: "servidor local", "Testar como servidor",
 * "Só cliente") e do protótipo aprovado (`design/prototipo-final/screens-test.js` →
 * `servidor-eula`, `servidor-opcoes`, `teste-servidor`). `console.*` são as linhas do Warden no
 * console do servidor (`servidor:console.*` no backend); `etapa.preparar` é o texto da etapa que
 * o backend manda.
 */
export const servidor = {
  menu: {
    testar: 'Testar como servidor…',
    testarDesc: 'Abre um servidor só neste computador e entra nele',
  },
  eula: {
    titulo: 'Aceitar a EULA do Minecraft?',
    texto: 'Para abrir um servidor do Minecraft, a Mojang exige que você aceite a EULA.',
    ler: 'Ler a EULA (aka.ms/MinecraftEULA)',
    lembrar:
      'O Warden pergunta só uma vez e lembra a sua resposta; dá para revogar em Configurações.',
    aceitar: 'Aceito a EULA',
    aceitando: 'Aceitando…',
    agoraNao: 'Agora não',
  },
  opcoes: {
    titulo: 'Testar como servidor',
    sub: 'Abre um servidor do Minecraft neste computador, só para você. Ele não faz parte do Testar normal.',
    carregando: 'Calculando a memória…',
    como: 'Como testar',
    comJogo: 'Abrir o servidor e entrar nele com o jogo',
    comJogoDesc: 'Testa os dois lados: o servidor com os mods e o jogo conectado a ele.',
    recomendado: 'Recomendado',
    so: 'Só o servidor',
    soDesc: 'Mais leve. Confere se o servidor abre com estes mods.',
    memoria: 'Memória',
    memoriaConta: 'servidor {{servidor}} + jogo {{jogo}} = {{total}}',
    memoriaSo: 'servidor {{servidor}}',
    livre: 'Livre agora',
    livreDesconhecida: 'o sistema não informou',
    poucaTitulo: 'Pode faltar memória.',
    poucaTexto:
      'Com {{livre}} livres, o computador pode ficar lento ou o jogo pode fechar. Feche outros programas, escolha "Só o servidor" ou use um perfil com menos memória.',
    poucaTextoSo:
      'Com {{livre}} livres, o computador pode ficar lento ou o servidor pode fechar. Feche outros programas antes de abrir.',
    loaderNao: 'O servidor local ainda não abre packs com Quilt ou LiteLoader.',
    local: 'Só aceita conexões deste computador (127.0.0.1) e fecha sozinho quando o teste acabar.',
    deFora_one: '{{count}} mod só de cliente fica de fora: {{nomes}}.',
    deFora_other: '{{count}} mods só de cliente ficam de fora: {{nomes}}.',
    maisN_one: '{{nomes}} e mais {{count}}',
    maisN_other: '{{nomes}} e mais {{count}}',
    nenhumDeFora: 'Nenhum mod é só de cliente: o pack inteiro vai para o servidor.',
    pasta: 'Usa uma pasta própria, separada da instância de teste. O pack não muda.',
    cancelar: 'Cancelar',
    abrir: 'Abrir servidor e testar',
  },
  etapas: {
    preparar: 'Preparar o servidor',
    abrir: 'Abrir o servidor e o jogo',
    abrirSo: 'Abrir o servidor',
  },
  etapa: {
    preparar: 'Preparando o servidor (na primeira vez, o Warden instala o servidor do loader)',
  },
  jogo: {
    titulo: 'Testando como servidor',
    sub: 'Servidor em 127.0.0.1, porta {{porta}}',
    subAbrindo: 'Abrindo o servidor em 127.0.0.1',
    subEntrou: 'o jogo entrou nele às {{hora}}',
    parar: 'Parar jogo e servidor',
    pararSo: 'Parar servidor',
    parando: 'Parando…',
    pararTitulo: 'Parar o jogo e o servidor?',
    pararTexto:
      'O jogo fecha na hora. O servidor recebe "stop" e, se não fechar em 60 s, é encerrado à força.',
    pararTituloSo: 'Parar o servidor?',
    pararTextoSo: 'O servidor recebe "stop" e, se não fechar em 60 s, é encerrado à força.',
    continuar: 'Continuar testando',
    mostrando: 'Mostrando',
    servidor: 'Servidor',
    jogo: 'Jogo',
    consoleServidor: 'Console do servidor',
    consoleJogo: 'Console do jogo',
    comando: 'Enviar comando ao servidor',
    comandoExemplo: 'Enviar comando ao servidor (ex.: say olá, reload)',
    comandoEsperando: 'Os comandos ficam disponíveis quando o servidor estiver pronto',
    enviar: 'Enviar',
  },
  console: {
    abrindo:
      'Abrindo o servidor: {{jogo}} · Java {{java}} · {{memoria}} · só em 127.0.0.1, porta {{porta}}',
    pronto: 'O servidor ficou pronto em {{duracao}} (porta {{porta}}).',
    parando: 'Enviando "stop" ao servidor…',
    fechou: 'O servidor fechou ({{duracao}} aberto).',
    travou: 'O servidor travou (código {{codigo}}, {{duracao}} aberto).',
  },
  resultado: {
    titulo: {
      servidorTravou: 'O servidor travou',
      servidorFechou: 'O servidor fechou normalmente',
      servidorEncerrado: 'Servidor encerrado por você',
    },
    lados: 'O que aconteceu em cada lado',
    servidor: 'Servidor',
    jogo: 'Jogo',
    servidorAbriu: 'Abriu em {{tempo}} e fechou normalmente.',
    servidorForcado: 'Abriu em {{tempo}}; não fechou com "stop" em 60 s e foi encerrado à força.',
    servidorTravouDepois: 'Abriu em {{tempo}} e depois travou.',
    servidorTravouAntes: 'Travou antes de ficar pronto.',
    servidorFechouAntes: 'Fechou antes de ficar pronto.',
    jogoEntrou: 'Entrou no servidor.',
    jogoNaoEntrou: 'Abriu, mas não entrou no servidor.',
    jogoNaoAbriu: 'Não abriu: o servidor travou antes de ficar pronto.',
    jogoSo: 'Não abriu (Só o servidor).',
    verConsole: 'Ver console do servidor',
    ocultarConsole: 'Ocultar console do servidor',
    carregandoConsole: 'Carregando o console do servidor…',
    porQue: 'Por que o servidor travou',
    modCliente: 'O mod {{mod}} é de cliente e derrubou o servidor.',
    modCulpado: 'O mod {{mod}} derrubou o servidor.',
    classe: 'Ele tentou carregar {{classe}}, que só existe no jogo.',
    marcarCliente: 'Para corrigir, marque o mod como "Só cliente" em Mods.',
    semCausa: 'Não encontramos a causa automaticamente. Veja o trecho do log do servidor abaixo.',
    trecho: 'Trecho do log do servidor',
  },
  configuracoes: {
    rotulo: 'EULA do Minecraft',
    aceitaEm: 'Aceita em {{data}}',
    aceita: 'Aceita',
    naoAceita: 'Ainda não aceita',
    revogar: 'Revogar',
    revogada: 'EULA revogada. O próximo servidor local vai perguntar de novo.',
    dica: 'Usada pelo servidor local (Testar como servidor). Revogar faz o Warden perguntar de novo.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    servidor: typeof servidor;
  }
}
