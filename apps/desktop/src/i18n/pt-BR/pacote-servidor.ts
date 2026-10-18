/**
 * "Pacote para servidor (.zip)" em Exportar (SPEC T19; ADR-0035; ROADMAP E-03; protótipo
 * `exportar-servidor` em `screens-pack.js`): as duas variantes, o que vai no pacote, a
 * validação no servidor local e o resultado. `etapa.*` são os textos das etapas que o backend
 * manda (`pacoteServidor:etapa.*`). Os textos da EULA e do "Por que o servidor travou" são os do
 * servidor local (`servidor`).
 */
export const pacoteServidor = {
  formato: {
    titulo: 'Pacote para servidor (.zip)',
    desc: 'Para abrir o pack num servidor (em casa ou numa hospedagem). Mods só de cliente ficam de fora.',
  },
  carregando: 'Lendo o que vai para o servidor…',
  servidorMinecraft: 'servidor do Minecraft',
  erroAnalise: 'Não foi possível ler o que vai para o servidor.',
  como: {
    titulo: 'Como os mods chegam ao servidor',
    recomendado: 'Recomendado',
    link: {
      titulo: 'Baixar os mods pelo link do pack',
      desc: 'O servidor baixa os mods ao iniciar e se atualiza sozinho a cada versão publicada. Precisa do pack publicado (está: versão {{versao}}).',
      semPublicacao:
        'O servidor baixa os mods ao iniciar e se atualiza sozinho a cada versão publicada. Precisa do pack publicado, e ele ainda não foi publicado.',
      privado:
        'O servidor baixa os mods ao iniciar pelo link do pack, mas o repositório do pack é privado ("só backup") e o link não abre para o servidor.',
      publicar: 'Publicar versão',
    },
    dentro: {
      titulo: 'Mods dentro do zip',
      desc: 'Funciona sem internet, mas o zip fica maior e não se atualiza: cada versão nova é um pacote novo.',
      manual_one:
        'Funciona sem internet, mas o zip fica maior e {{count}} mod da CurseForge precisa ser baixado à mão.',
      manual_other:
        'Funciona sem internet, mas o zip fica maior e {{count}} mods da CurseForge precisam ser baixados à mão.',
    },
    memoria: 'Memória do servidor (user_jvm_args.txt)',
    memoriaRecomendada: '{{memoria}} (recomendada para este pack)',
  },
  conteudo: {
    titulo: 'O que vai no pacote',
    scripts:
      'start.bat e start.sh: conferem o Java, perguntam sobre a EULA, instalam o {{loader}} na primeira vez e abrem o servidor.',
    memoria: 'user_jvm_args.txt: -Xms{{valor}} -Xmx{{valor}}',
    bootstrap:
      'packwiz-installer-bootstrap.jar e packwiz-installer.jar: baixam os mods do lado servidor pelo link, a cada vez que o servidor abre.',
    itensLink_one: '{{count}} mod ou datapack do lado servidor, baixado pelo link.',
    itensLink_other: '{{count}} mods e datapacks do lado servidor, baixados pelo link.',
    itensDentro_one: '{{count}} mod ou datapack do lado servidor, dentro do zip.',
    itensDentro_other: '{{count}} mods e datapacks do lado servidor, dentro do zip.',
    pasta_one: '{{pasta}}/: {{count}} arquivo',
    pasta_other: '{{pasta}}/: {{count}} arquivos',
    raiz_one: '{{count}} arquivo solto na raiz',
    raiz_other: '{{count}} arquivos soltos na raiz',
    semClientScripts: '(sem client_scripts/)',
    overridesLink:
      'O que está em server-overrides/ vai como pasta e é copiado por cima do pack sempre que o servidor abre.',
    overridesDentro_one:
      '{{count}} arquivo de server-overrides/ já vai no lugar do arquivo do pack.',
    overridesDentro_other:
      '{{count}} arquivos de server-overrides/ já vão no lugar dos arquivos do pack.',
    tambemDeFora: 'Também ficam de fora options.txt, resource packs e shaders.',
    eula: 'O pacote não aceita a EULA por ninguém: o script pergunta a quem roda o servidor.',
    versaoNova: 'Se uma versão nova do pack mudar o Minecraft ou o loader, gere o pacote de novo.',
  },
  loaderNao: 'O pacote para servidor ainda não sai para packs com {{loader}}.',
  semChave: {
    titulo: 'Falta a chave da CurseForge',
    texto:
      'Com mods dentro do zip, o Warden precisa da chave da CurseForge para baixar os mods de lá que ainda não estão neste computador. Digite a chave em Configurações.',
  },
  manual: {
    titulo_one: '{{count}} mod da CurseForge vai precisar ser baixado à mão',
    titulo_other: '{{count}} mods da CurseForge vão precisar ser baixados à mão',
    texto:
      'O autor não deixa outros apps baixarem: {{nomes}}. Ao gerar, a lista aparece aqui com a página de download de cada um.',
  },
  licenca: {
    titulo_one: '{{count}} arquivo de terceiros vai dentro do zip',
    titulo_other: '{{count}} arquivos de terceiros vão dentro do zip',
    texto:
      'Os mods vão dentro do pacote, e quem recebe o zip ganha uma cópia de cada um. Pelo link, ninguém recebe cópia: o servidor baixa de onde o pack aponta.',
    confirmar: 'Tenho o direito de distribuir estes arquivos',
    confirmarDesc: 'A licença de cada mod permite que outras pessoas recebam uma cópia.',
  },
  naoTestado: {
    titulo: 'Este pacote ainda não foi testado num servidor.',
    texto:
      'Às vezes um mod diz que funciona no servidor e não funciona. Testar abre um servidor neste computador, só para você.',
    testar: 'Testar como servidor',
  },
  validar: {
    rotulo: 'Testar no servidor deste computador depois de gerar (recomendado)',
    desc: 'O Warden abre o pacote num servidor só deste computador até ele ficar pronto e fecha em seguida.',
  },
  acao: {
    gerar: 'Gerar pacote para servidor…',
    dica: 'Você escolhe onde salvar o arquivo .zip.',
    somenteLeitura: 'Este pack é somente leitura, mas o pacote pode ser gerado mesmo assim.',
    falha: 'Não foi possível gerar o pacote para servidor.',
    confirmar: 'Confirme os arquivos de terceiros acima para gerar com mods dentro.',
  },
  andamento: {
    rotulo: 'Gerando o pacote para servidor',
    meta: 'Montando e conferindo o pacote',
    aguardando: 'Aguardando outra operação neste pack',
    cancelar: 'Cancelar',
  },
  etapa: {
    mods: 'Baixando e conferindo os mods do servidor',
    montar: 'Montando o pacote',
    testar: 'Testando o pacote no servidor deste computador',
  },
  resultado: {
    titulo: 'Pacote para servidor gerado',
    resumo_one: '{{count}} arquivo ({{tamanho}}), salvo em',
    resumo_other: '{{count}} arquivos ({{tamanho}}), salvo em',
    mods_one: 'Com {{count}} mod dentro do zip.',
    mods_other: 'Com {{count}} mods dentro do zip.',
    sha: 'SHA-256',
    testado: 'Pacote para servidor testado: o servidor abriu em {{tempo}}.',
    travou: 'O servidor travou com este pacote.',
    travouTexto: 'O pacote foi gerado, mas o servidor fechou antes de ficar pronto.',
    naoTestou: 'Não foi possível testar o pacote neste computador.',
    naoTestado: 'Este pacote ainda não foi testado num servidor.',
    passos: {
      titulo: 'Como usar no servidor',
      rotulo: 'Passos para abrir o servidor com o pacote',
      um: 'Descompacte o zip numa pasta vazia do servidor.',
      dois: 'Rode start.bat (Windows) ou start.sh (Linux e macOS).',
      tres: 'Responda "s" para aceitar a EULA do Minecraft. Na primeira vez, o script instala o {{loader}}.',
      link: 'A cada vez que abre, o servidor baixa pelo link os mods da última versão publicada.',
    },
    mostrar: 'Mostrar arquivo',
    fechar: 'Fechar',
  },
};

declare module '../catalogo' {
  interface Catalogo {
    pacoteServidor: typeof pacoteServidor;
  }
}
