/**
 * Mods iniciais do Criar pack (SPEC T03, etapa 4), kits de desempenho (T03 e T08) e o aviso
 * das ferramentas do jogador (ADR-0033, decisão D16). Protótipo aprovado:
 * `design/prototipo-final/screens-app.js` (`criar-4`). Glossário: QUALITY §8.2.
 */
export const modsIniciais = {
  intro:
    'Mods que ajudam você a testar e ajudam quem joga quando algo dá errado. Vêm marcados; desmarque o que não quiser. Entram como qualquer mod, logo depois de o pack ser criado.',
  recomendados: 'Recomendados para Minecraft {{minecraft}} com {{loader}}',
  semLoader: {
    titulo: 'Este pack não tem loader',
    texto:
      'Mods precisam de um loader. Você pode criar o pack agora e adicionar resource packs e shaders depois.',
  },
  carregando: 'Consultando os mods iniciais…',
  erro: {
    titulo: 'Não foi possível consultar os mods iniciais agora',
    texto:
      'Sem acesso ao Modrinth. Crie o pack sem eles e adicione-os depois pela página Mods, ou tente de novo.',
    tentar: 'Tentar de novo',
  },
  nenhum: {
    titulo: 'Nenhum mod inicial para esta versão',
    texto: 'Não há spark nem Crash Assistant para este Minecraft e este loader.',
  },
  ferramenta: {
    spark: {
      nome: 'spark',
      descricao: 'Mede o que deixa o jogo lento. Lado: cliente e servidor.',
    },
    'crash-assistant': {
      nome: 'Crash Assistant',
      descricao:
        'Mostra uma janela clara para o jogador quando o jogo trava. O envio de dados ao autor do mod vem desligado. Fica fora dos seus testes normais.',
    },
  },
  selecionar: 'Adicionar {{name}}',
  versao: 'Versão {{version}}',
  junto: 'Traz junto: {{names}}',
  antiga: 'Versão antiga, sem atualizações',
  antigaDica:
    'O spark para esta versão do Minecraft é antigo e não recebe mais atualizações. Ainda mede o desempenho.',
  semChave: 'Precisa da chave da CurseForge',
  semChaveDica:
    'Este item só existe na CurseForge. Coloque a chave nas Configurações para poder adicioná-lo. O pack é criado do mesmo jeito.',
  semVersao: 'Sem versão para este pack',
  kit: {
    legenda: 'Kit de desempenho',
    opcional: '(opcional)',
    nenhum: 'Sem kit de desempenho para esta versão do Minecraft e este loader.',
    adicionar: 'Adicionar o kit {{name}}',
    maisQuantos_one: 'e mais {{count}}',
    maisQuantos_other: 'e mais {{count}}',
    ver: 'Ver o que vem no kit',
    esconder: 'Esconder o que vem no kit',
    itens: 'Mods do kit {{name}}',
    marcarItem: 'Incluir {{name}}',
    confirma:
      'Os mods marcados e as dependências obrigatórias deles entram logo depois de o pack ser criado.',
    marcarParaEditar: 'Marque o kit para escolher os mods.',
    notas: {
      worldgen: 'Muda a geração do mundo. Vem desmarcado.',
      shaders: 'Só para quem usa shaders. Vem desmarcado.',
    },
    lado: {
      both: 'Cliente e servidor',
      client: 'Só cliente',
      server: 'Só servidor',
    },
  },
  kits: {
    titulo: 'Kits de desempenho',
    texto:
      'Listas de mods que deixam o jogo mais leve sem mudar como ele é jogado. Nada entra sem a sua confirmação.',
    carregando: 'Carregando os kits',
    nenhum: 'Não há kit de desempenho para Minecraft {{minecraft}} com {{loader}}.',
    adicionarKit: 'Adicionar kit',
    semItens: 'Marque ao menos um mod do kit.',
    itensMarcados_one: '{{count}} mod marcado',
    itensMarcados_other: '{{count}} mods marcados',
  },
  resumo: {
    titulo: 'Mods iniciais',
    nenhum: 'Nenhum mod inicial',
    lista: 'Mods iniciais: {{nomes}}',
    config: 'config do Crash Assistant sem envio de dados ao autor',
    kit: 'kit {{name}} ({{count}} mods)',
  },
  resultado: {
    feito_one: '{{count}} mod inicial adicionado',
    feito_other: '{{count}} mods iniciais adicionados',
    ficaramDeFora: 'Ficaram de fora: {{nomes}}. Dá para adicionar depois pela página Mods.',
    falhou: 'O pack foi criado, mas os mods iniciais não entraram. Adicione-os pela página Mods.',
  },
} as const;
