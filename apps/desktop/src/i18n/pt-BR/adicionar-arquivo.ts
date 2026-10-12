/**
 * Link e arquivo do computador na página Adicionar (SPEC T08 "Link" e "Arquivo"; P1-11):
 * links do Modrinth e diretos, `http://` recusado, arquivos escolhidos ou arrastados, a
 * identificação pelo hash e o bloqueio de loader ou versão com "Adicionar mesmo assim".
 * O link da CurseForge está em `adicionar-curseforge.ts`. Glossário: QUALITY §8.2.
 */
export const adicionarArquivo = {
  link: {
    recusadoHttp: 'Links http:// não são aceitos.',
    recusadoHttpTexto:
      'O Warden só baixa por https://, que protege o arquivo no caminho. Nada foi baixado. Procure o link https:// do mesmo arquivo.',
    naoSuportado: 'O Warden não sabe adicionar este link.',
    naoSuportadoTexto:
      'Cole o link de um projeto ou versão do Modrinth, de um projeto ou arquivo da CurseForge, ou um link https:// que termine em .jar ou .zip.',
    lendoModrinth: 'Lendo o link do Modrinth…',
    erroModrinth: 'Não foi possível ler este link do Modrinth.',
    tipoNaoAceito:
      'Este projeto não é mod, resource pack nem shader. A página Adicionar só aceita esses tipos.',
    projetoLido: 'Link de {{name}} lido.',
    versaoLida: 'Link de uma versão de {{name}} lido.',
    verProjeto: 'Ver {{name}}',
    adicionarVersao: 'Adicionar esta versão de {{name}}',
    baixando: 'Baixando {{file}}…',
    baixandoTexto:
      'O arquivo é baixado uma vez, para o Warden calcular o hash e ler os metadados. Nada foi gravado no pack.',
    cancelar: 'Cancelar download',
    cancelado: 'Download cancelado. Nada foi gravado no pack.',
    erroDownload: 'Não foi possível baixar este link.',
    tentarDeNovo: 'Baixar de novo',
  },
  arquivo: {
    escolher: 'Escolher arquivo do computador…',
    lista: 'Arquivos para adicionar',
    voltar: 'Voltar para a busca',
    lendo_one: 'Lendo {{count}} arquivo e procurando no Modrinth e na CurseForge…',
    lendo_other: 'Lendo {{count}} arquivos e procurando no Modrinth e na CurseForge…',
    erroLeitura: 'Não foi possível ler os arquivos.',
    ignorados_one: '{{count}} item não é .jar nem .zip e ficou de fora.',
    ignorados_other: '{{count}} itens não são .jar nem .zip e ficaram de fora.',
    nenhum: 'Nenhum arquivo .jar ou .zip foi escolhido.',
    doLink: 'Baixado de {{host}}',
    loaders: 'Para {{loaders}}',
    minecraft: 'Minecraft {{range}}',
    tipo: 'Tipo',
    tipoOpcoes: {
      mod: 'Mod',
      resourcePack: 'Resource pack',
      shader: 'Shader',
    },
    remover: 'Tirar {{file}} da lista',
    naoEZip: 'Este arquivo não é um .jar ou .zip legível. Confira se o download terminou.',
  },
  identificado: {
    tituloModrinth: 'Este arquivo é o {{name}} {{version}} do Modrinth.',
    tituloCurseforge: 'Este arquivo é o {{name}} {{version}} da CurseForge.',
    texto:
      'Como referência, o pack guarda só o endereço oficial: quem joga baixa direto da fonte, e as atualizações do Warden funcionam.',
    referenciaModrinth: 'Adicionar como referência ao Modrinth (recomendado)',
    referenciaCurseforge: 'Adicionar como referência à CurseForge (recomendado)',
    comoArquivo: 'Adicionar como arquivo local',
    comoLink: 'Adicionar pelo link',
    naoAchado: 'Este arquivo não foi achado no Modrinth nem na CurseForge.',
    naoAchadoTexto: 'Ele entra como arquivo local, copiado para a pasta {{pasta}} do pack.',
    naoAchadoLinkTexto:
      'Ele entra pelo link: o pack guarda o endereço e o hash, e quem joga baixa desse endereço.',
    falhou:
      'Não foi possível procurar em {{fontes}} agora. O arquivo ainda pode entrar como arquivo local.',
    adicionarArquivo: 'Adicionar {{file}}',
    adicionarLink: 'Adicionar pelo link',
  },
  bloqueio: {
    titulo: 'Este arquivo não serve para este pack.',
    loader: 'Ele é um mod para {{found}}, e este pack usa {{pack}}.',
    minecraft: 'Ele é para o Minecraft {{accepts}}, e este pack é do Minecraft {{pack}}.',
    referenciaIncompativel: 'Pelos dados da fonte, esta versão também não é para {{alvo}}.',
    texto:
      'Se adicionar mesmo assim, o jogo provavelmente não abre, e o arquivo aparece como problema no diagnóstico.',
    mesmoAssim: 'Adicionar mesmo assim',
    aceito: 'Você escolheu adicionar mesmo assim. Ele vai aparecer como problema no diagnóstico.',
  },
  noPack: {
    mesmoArquivo: 'Este arquivo já está no pack ({{name}}).',
    mesmoNome:
      'Já existe no pack um arquivo com este nome ({{path}}). Renomeie o arquivo ou remova o outro antes.',
  },
  lado: {
    desconhecido: 'Lado desconhecido — confira se é só de cliente.',
  },
  fonte: {
    modrinth: 'Modrinth',
    curseforge: 'CurseForge',
  },
  adicionadoComo: 'Adicionado ao pack ({{path}}).',
  adicionado_one: '{{count}} arquivo adicionado ao pack.',
  adicionado_other: '{{count}} arquivos adicionados ao pack.',
  erros: {
    changed: 'O arquivo {{name}} mudou depois de lido. Escolha o arquivo de novo.',
    nameTaken: 'Já existe no pack um arquivo com o nome {{name}}.',
    incompatible: '{{name}} não serve para este pack. Use “Adicionar mesmo assim” para continuar.',
    tooLarge: 'O arquivo {{name}} passa de 256 MB, o maior tamanho aceito.',
    notFound: 'O arquivo {{name}} não existe mais. Escolha o arquivo de novo.',
    unreadable: 'Não foi possível ler o arquivo {{name}}.',
    badName: 'O nome do arquivo {{name}} não pode ser usado no pack. Renomeie o arquivo.',
    notFile: '{{name}} não é um arquivo.',
    expired: 'O arquivo escolhido expirou. Escolha o arquivo de novo.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    adicionarArquivo: typeof adicionarArquivo;
  }
}
