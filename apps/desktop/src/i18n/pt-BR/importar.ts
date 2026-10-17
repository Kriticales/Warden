/**
 * Importar modpack (SPEC T24; ADR-0035; protótipo `importar`): a página com o que foi
 * reconhecido, nome e pasta do pack novo, o resumo da conversão e o que não entra; e o aviso
 * "N itens para revisar" em Problemas, depois de importar.
 */
export const importar = {
  titulo: 'Importar modpack',
  sub: 'Vira um pack packwiz novo. O arquivo original não muda.',
  subPronto: '{{arquivo}} · Minecraft {{minecraft}} · {{loader}}',
  vanilla: 'Vanilla',
  voltar: 'Meus packs',
  onde: 'Abrir ou importar',
  formato: {
    packwiz: 'pack packwiz',
    mrpack: '.mrpack',
    curseforgeZip: 'zip da CurseForge',
    prism: 'instância do Prism ou do MultiMC',
    curseforgeApp: 'instância do app da CurseForge',
  },
  reconhecido: {
    titulo: 'O que foi reconhecido',
    formato: 'Formato',
    nome: 'Modpack',
    versao: 'Versão',
    semVersao: 'sem versão',
  },
  lendo: 'Lendo {{arquivo}}',
  lendoMeta: 'Identificando os arquivos no Modrinth e na CurseForge pelo hash',
  cancelar: 'Cancelar',
  escolherOutro: 'Escolher outro arquivo',
  abrirConfiguracoes: 'Abrir Configurações',
  semChave: {
    titulo: 'Para importar um modpack da CurseForge, o Warden precisa da sua chave da CurseForge.',
    texto:
      'O zip só traz os números dos mods; a chave é usada para saber quais mods são. Nada foi gravado.',
  },
  loader: {
    naoSuportado: 'Este modpack usa {{loader}}, que o Warden ainda não suporta.',
    varios: 'Este modpack declara mais de um loader e não pode ser importado.',
    texto: 'Nada foi criado. Modpacks Forge, NeoForge e Fabric são importados normalmente.',
  },
  erro: 'Não foi possível ler este modpack.',
  licenca: {
    titulo: 'Licença do modpack: {{licenca}}.',
    livre: 'É uma licença livre: dá para usar e adaptar o modpack, respeitando os termos dela.',
    reservada:
      'As configs e os arquivos são trabalho do autor. Para uso pessoal, tudo bem; para redistribuir, peça permissão.',
  },
  lojas: {
    titulo: 'Não foi possível consultar {{lojas}}.',
    texto:
      'Os jars que não puderam ser conferidos ficam como arquivo local. Confira a conexão com a internet e leia o modpack de novo para tentar outra vez.',
    reler: 'Ler de novo',
  },
  destino: 'Nome e pasta do pack novo',
  resumo: {
    titulo: 'O que acontece com cada arquivo',
    verLista: 'Ver lista',
    modrinth_one: '{{count}} vira referência ao Modrinth',
    modrinth_other: '{{count}} viram referência ao Modrinth',
    modrinthTexto: 'Baixados pelo jogador direto do Modrinth, como num pack feito no Warden.',
    curseforge_one: '{{count}} vira referência à CurseForge',
    curseforge_other: '{{count}} viram referência à CurseForge',
    curseforgeTexto: 'Baixados pela CurseForge, como num pack feito no Warden.',
    link_one: '{{count}} vira link direto',
    link_other: '{{count}} viram link direto',
    linkTexto: 'Arquivos de outros sites, baixados pelo link e conferidos pelo hash.',
    hash_one: '{{count}} jar solto identificado pelo hash',
    hash_other: '{{count}} jars soltos identificados pelo hash',
    hashTexto: 'Estavam dentro do modpack e também viram referência (já contados acima).',
    locais_one: '{{count}} jar fica como arquivo local',
    locais_other: '{{count}} jars ficam como arquivo local',
    locaisTexto:
      'Não viraram referência. Vão dentro do pack; confira se a licença de cada um permite.',
    revisar_one: '{{count}} item para revisar',
    revisar_other: '{{count}} itens para revisar',
    revisarTexto: 'Entram no pack e ficam marcados para você conferir.',
    arquivos_one: '{{count}} arquivo do modpack (configs, scripts e outros)',
    arquivos_other: '{{count}} arquivos do modpack (configs, scripts e outros)',
    arquivosTexto: 'Copiados para o pack como estão.',
    servidor_one: '{{count}} arquivo só para o servidor',
    servidor_other: '{{count}} arquivos só para o servidor',
    servidorTexto: 'Vão para a pasta server-overrides/, que só o pacote para servidor usa.',
  },
  fonte: {
    modrinth: 'Modrinth',
    curseforge: 'CurseForge',
    url: 'link direto',
  },
  motivoLocal: {
    notFound: 'não está no Modrinth nem na CurseForge',
    noCurseforgeKey: 'não está no Modrinth; a CurseForge não foi consultada (sem chave)',
    lookupFailed: 'a consulta pelo hash falhou',
  },
  motivoRevisar: {
    unknownSide: 'lado desconhecido no modpack: entra como Cliente e servidor',
    optional: 'opcional no modpack: entra opcional e marcado',
    optionalDisabled: 'opcional desligado no modpack: entra opcional e desmarcado',
    clientOverride: 'veio de client-overrides/: entra para todos, porque o packwiz não separa',
  },
  lixo: {
    titulo: 'O que não entra no pack',
    texto:
      'Dados de execução, arquivos de outros launchers e lixo encontrados pela verificação de higiene. Os itens desconhecidos podem entrar, se você marcar.',
    arquivos_one: '{{count}} arquivo',
    arquivos_other: '{{count}} arquivos',
    levar: 'Levar {{path}} para o pack',
    motivo: {
      runtime: 'dados de execução do jogo ou do launcher',
      tools: 'segredo ou arquivo de ferramenta',
      junk: 'cópia de segurança, log ou arquivo desligado',
      serverData: 'dado de servidor ou do instalador',
      otherLauncher: 'arquivo de outro launcher',
      unknownRoot: 'pasta ou arquivo desconhecido na raiz',
      symlink: 'link simbólico',
    },
  },
  perfil: {
    titulo: 'Perfil do teste',
    texto:
      'A memória e os argumentos do modpack viram um perfil do teste, já escolhido para o próximo teste.',
    memoria: 'Memória: {{mb}} MB',
    memoriaAuto: 'Memória: automática',
    argumentos: 'Argumentos: {{args}}',
  },
  preferencias: {
    titulo: 'Levar suas preferências para o pack?',
    texto:
      'As opções do jogo e a lista de servidores da instância ({{arquivos}}) iriam para todo mundo que jogar o pack.',
    levar: 'Levar {{arquivos}} para o pack',
  },
  importar: 'Importar como pack novo',
  importando: 'Importando…',
  feito: '“{{name}}” importado como pack novo.',
  feitoRevisar_one: '{{count}} item marcado para revisar aparece em Problemas.',
  feitoRevisar_other: '{{count}} itens marcados para revisar aparecem em Problemas.',
  semPerfil:
    'O perfil do teste com a memória do modpack não pôde ser criado. Ajuste a memória em Testar.',
  revisao: {
    titulo_one: '{{count}} item para revisar',
    titulo_other: '{{count}} itens para revisar',
    texto:
      'A importação de {{arquivo}} marcou estes itens para você conferir. Quando terminar, clique em Já revisei.',
    pronto: 'Já revisei',
    revisarCurto: 'Revisar',
    revisar: 'Revisar {{name}}',
    motivo: {
      unknownSide: 'lado desconhecido no modpack: entrou como Cliente e servidor',
      optional: 'opcional no modpack: entrou opcional e marcado',
      optionalDisabled: 'opcional desligado no modpack: entrou opcional e desmarcado',
      clientOverride: 'veio de client-overrides/: entrou para todos, porque o packwiz não separa',
      localFile: 'ficou como arquivo local: confira se a licença permite levá-lo no pack',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    importar: typeof importar;
  }
}
