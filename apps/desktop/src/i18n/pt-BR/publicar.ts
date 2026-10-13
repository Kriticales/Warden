/**
 * Publicar versão no GitHub para os jogadores (SPEC T18; V-03): a área "Publicação para os
 * jogadores" do Histórico, a primeira publicação, o diálogo com os avisos e as notas, e o
 * resultado com o link do pack e o passo a passo dos jogadores.
 *
 * Glossário (QUALITY §8): "versão final", "publicar", "link do pack", "jogadores"; o dono não vê
 * git (nada de commit, push ou branch).
 */
export const publicar = {
  area: {
    titulo: 'Publicação para os jogadores',
    antes:
      'Este pack ainda não foi publicado. Salve uma versão marcada como versão final e publique: os jogadores recebem pelo link do pack.',
    carregando: 'Lendo a publicação…',
    erro: 'Não foi possível ler a publicação do pack.',
    repositorio: 'Repositório',
    publico: 'Público',
    privado: 'Privado',
    versaoPublicada: 'Versão publicada',
    publicadaEm: '{{version}}, em {{date}}',
    nenhumaPublicada: 'Nenhuma versão publicada ainda',
    link: 'Link do pack',
    linkDica: 'Quem joga com este link recebe as atualizações sozinho ao abrir o jogo.',
    soBackup: 'Só backup: o link não funciona para os jogadores.',
    comoInstalam: 'Como os jogadores instalam',
    releasePendente_one:
      'A versão {{versions}} foi enviada, mas a Release do GitHub não foi criada.',
    releasePendente_other:
      'As versões {{versions}} foram enviadas, mas as Releases do GitHub não foram criadas.',
    tentarDeNovo: 'Tentar de novo',
    tentarDeNovoVersao: 'Tentar de novo ({{version}})',
    releaseCriada: 'Release da versão {{version}} criada no GitHub.',
    releaseFalhou: 'A Release não foi criada',
  },
  copiar: {
    link: 'Copiar link',
    copiado: 'Link copiado.',
    instrucoes: 'Copiar instruções',
    instrucoesCopiadas: 'Instruções copiadas.',
    notas: 'Copiar texto das notas da versão',
    notasCopiadas: 'Notas copiadas.',
    falhou: 'Não foi possível copiar. Selecione o texto e copie à mão.',
  },
  botao: {
    publicar: 'Publicar versão {{version}}',
  },
  token: {
    titulo: 'Falta o token do GitHub',
    texto:
      'Para publicar, o Warden precisa de um token do GitHub. Crie o token com o passo a passo de Configurações → Chaves e contas e salve lá.',
    abrir: 'Abrir Configurações',
  },
  repo: {
    titulo: 'Primeira publicação deste pack',
    descricao: 'O Warden cria um repositório no seu GitHub para o pack.',
    nome: 'Nome do repositório',
    nomeDica: 'Letras sem acento, números, ponto, hífen ou sublinhado.',
    nomeInvalido: 'Use só letras sem acento, números, ponto, hífen ou sublinhado.',
    quemVe: 'Quem pode ver',
    recomendado: 'Recomendado',
    publico: 'Público',
    publicoDesc:
      'Os jogadores recebem atualizações automáticas pelo link do pack. Qualquer pessoa com o link vê os arquivos do pack (mods e configs). Nada do seu computador vai junto.',
    privado: 'Privado',
    privadoDesc:
      'Só backup no GitHub. O link não funciona para os jogadores: eles não recebem atualizações automáticas.',
    usarExistente: 'Usar um repositório que já existe',
    criarNovo: 'Criar um repositório novo',
    existente: 'Repositório (dono/nome)',
    existenteDica:
      'Por exemplo: seu-usuario/meu-pack. O Warden mostra se ele é público ou privado.',
    criar: 'Criar repositório e continuar',
    vincular: 'Usar este repositório e continuar',
    cancelar: 'Cancelar',
    vinculadoPrivado:
      'O repositório {{repo}} é privado: só backup. O link não funciona para os jogadores.',
  },
  plano: {
    titulo: 'Publicar a versão {{version}} para os jogadores',
    descricao:
      'Vai para o GitHub ({{repo}}). Quem joga com o link do pack recebe a atualização ao abrir o jogo.',
    carregando: 'Conferindo a versão {{version}}…',
    erro: 'Não foi possível conferir a versão.',
    bloqueios: 'Antes de publicar, resolva',
    bloqueiosDica:
      'Publicar fica bloqueado até resolver: corrija o pack, salve uma versão nova e publique essa.',
    avisos: 'Avisos',
    semAvisos: 'Nenhum aviso: a versão foi testada e os mods baixam sozinhos para os jogadores.',
    trocaMudaPack:
      'Trocar um mod muda o pack: depois é preciso salvar uma versão nova e publicar essa.',
    notas: 'Notas da versão',
    notasDesde:
      'Mudanças desde a {{version}}, a última publicada. Vão para o CHANGELOG.md e para a Release do GitHub.',
    notasPrimeira:
      'Primeira publicação: tudo o que o pack tem. Vai para o CHANGELOG.md e para a Release do GitHub.',
    notasVazias: 'Nenhuma mudança registrada.',
    recado: 'Notas para os jogadores',
    opcional: '(opcional)',
    recadoPlaceholder: 'Ex.: façam backup dos mundos antes de atualizar',
    vai: 'Vai para o GitHub',
    vaiTexto_one:
      'pack.toml, index.toml, {{count}} arquivo do índice, CHANGELOG.md e .gitattributes',
    vaiTexto_other:
      'pack.toml, index.toml, {{count}} arquivos do índice, CHANGELOG.md e .gitattributes',
    naoVai: 'Não vai',
    naoVaiTexto: '.warden/, mundos de teste, registros, versões não publicadas',
    cancelar: 'Cancelar',
    publicar: 'Publicar',
    publicarMesmoAssim: 'Publicar mesmo assim',
    publicando: 'Publicando…',
    bloqueado: 'Resolva o que bloqueia a publicação antes',
  },
  achado: {
    segredo:
      'Encontramos algo que parece uma chave em {{path}}, linha {{line}}. Remova antes de publicar.',
    higiene: '{{path}} não pode ir para os jogadores.',
    limpar: 'Limpar',
    limpo_one: '{{count}} arquivo limpo. Salve uma versão nova e publique essa.',
    limpo_other: '{{count}} arquivos limpos. Salve uma versão nova e publique essa.',
    bloqueados_one:
      '{{count}} mod da CurseForge não pode ser baixado automaticamente pelos jogadores.',
    bloqueados_other:
      '{{count}} mods da CurseForge não podem ser baixados automaticamente pelos jogadores.',
    bloqueadosTexto:
      'O autor bloqueou downloads por outros apps. Cada jogador vai ter que baixar à mão.',
    soCurseforge_one:
      '{{count}} mod só existe na CurseForge e pode deixar de baixar para os jogadores se a CurseForge passar a exigir chave nos downloads.',
    soCurseforge_other:
      '{{count}} mods só existem na CurseForge e podem deixar de baixar para os jogadores se a CurseForge passar a exigir chave nos downloads.',
    mesmoArquivo: 'o mesmo arquivo existe no Modrinth',
    outraVersao: 'no Modrinth: {{arquivo}} (versão {{versao}})',
    naoExiste: 'não existe no Modrinth',
    trocar: 'Trocar pelo Modrinth',
    trocarNome: 'Trocar {{nome}} pelo Modrinth',
    trocado: '{{nome}} trocado pelo Modrinth. Salve uma versão nova e publique essa.',
    trocaFalhou: 'Não foi possível trocar {{nome}}',
    naoTestada: 'A versão {{version}} não foi testada.',
    naoTestadaTexto: 'Nenhum teste que fechou normalmente usou exatamente o conteúdo desta versão.',
    testarAgora: 'Testar agora',
    semChave:
      'Sem a chave da CurseForge, o Warden não conseguiu conferir se os mods da CurseForge baixam para os jogadores.',
    semConexao:
      'Não foi possível consultar a CurseForge e o Modrinth agora. Os avisos sobre mods podem estar incompletos.',
  },
  divergencia: {
    titulo: 'O GitHub tem mudanças que não estão aqui',
    texto:
      'A página do pack no GitHub mudou por fora do Warden. Publicar substitui o GitHub pelo que está aqui; o que foi feito lá se perde.',
    substituir: 'Substituir o GitHub pelo que está aqui',
    confirmarTitulo: 'Substituir o GitHub pelo que está aqui?',
    confirmarTexto:
      'O que foi mudado no GitHub por fora do Warden se perde, e os jogadores recebem a versão {{version}}.',
    confirmar: 'Substituir e publicar',
    substituindo: 'Substituindo…',
  },
  resultado: {
    titulo: 'Versão {{version}} publicada',
    descricao:
      'Quem já joga com o link recebe a atualização na próxima vez que abrir o jogo. O GitHub pode levar alguns minutos para mostrar a versão nova.',
    abrirRelease: 'Abrir a Release no GitHub',
    fechar: 'Fechar',
  },
  passos: {
    titulo: 'Como os jogadores instalam (Prism Launcher ou MultiMC)',
    criar: 'Criar uma instância com Minecraft {{minecraft}} e {{loader}}.',
    criarSemLoader: 'Criar uma instância com Minecraft {{minecraft}}.',
    baixar: 'Baixar o packwiz-installer-bootstrap.jar e colocar na pasta .minecraft da instância.',
    comando:
      'Em Editar instância → Configurações → Comandos personalizados, colar no comando antes de iniciar:',
    abrir: 'Abrir o jogo. O pack é baixado agora e atualizado sozinho nas próximas vezes.',
    ajuda403:
      'Se aparecer um erro 403, o GitHub limitou os downloads por um tempo. Abra o jogo de novo mais tarde ou baixe também o packwiz-installer.jar e coloque ao lado do bootstrap.',
    fechar: 'Fechar',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    publicar: typeof publicar;
  }
}
