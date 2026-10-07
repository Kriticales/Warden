/**
 * "Instância pronta para o Prism" (SPEC T19 e T18; ADR-0050): o formato em Exportar, a análise
 * antes de gerar, o resultado e o passo a passo para o jogador.
 */
export const instanciaPrism = {
  formato: {
    titulo: 'Instância pronta para o Prism (.mrpack)',
    desc: 'Um arquivo que o jogador arrasta para o Prism Launcher: o Prism cria a instância e baixa os mods sozinho.',
  },
  carregando: 'Conferindo os mods do pack…',
  erroAnalise: 'Não foi possível conferir os mods do pack.',
  explicacao: {
    titulo: 'O que o jogador vai ver',
    texto:
      'O Prism vai pedir para o jogador confirmar os mods que não vêm do Modrinth. Esta instância não se atualiza sozinha: cada versão nova é um arquivo novo.',
  },
  resumo: {
    titulo: 'O que vai no arquivo',
    jogo: 'Minecraft {{minecraft}}{{loader}}, versão {{versao}} do pack.',
    loader: ' com {{loader}}',
    modrinth_one: '{{count}} arquivo do Modrinth, baixado pelo Prism no Modrinth.',
    modrinth_other: '{{count}} arquivos do Modrinth, baixados pelo Prism no Modrinth.',
    curseforge_one:
      '{{count}} arquivo da CurseForge, baixado pelo Prism com a chave do próprio jogador.',
    curseforge_other:
      '{{count}} arquivos da CurseForge, baixados pelo Prism com a chave do próprio jogador.',
    links_one: '{{count}} arquivo de link direto.',
    links_other: '{{count}} arquivos de link direto.',
    overrides_one: '{{count}} arquivo do pack vai dentro do arquivo (configs, scripts e locais).',
    overrides_other:
      '{{count}} arquivos do pack vão dentro do arquivo (configs, scripts e locais).',
    naoConfiaveis_one: 'O Prism vai pedir confirmação de {{count}} item.',
    naoConfiaveis_other: 'O Prism vai pedir confirmação de {{count}} itens.',
    faltandoJars:
      'Os arquivos que ainda não estão neste computador são baixados antes de gerar, para conferir e registrar cada um.',
  },
  semVersao: {
    titulo: 'O pack ainda não tem versão',
    texto:
      'A instância pronta precisa da versão do pack. Salve uma versão (Salvar versão, no alto) e volte aqui.',
  },
  semChave: {
    titulo: 'Falta a chave da CurseForge',
    texto:
      'O pack tem mods da CurseForge e o Warden precisa da chave para conferir se o autor deixa apps de terceiros baixarem. Digite a chave em Configurações.',
  },
  bloqueados: {
    titulo_one: '{{count}} mod da CurseForge não pode ser baixado pelo Prism',
    titulo_other: '{{count}} mods da CurseForge não podem ser baixados pelo Prism',
    texto:
      'O autor desligou o download por apps de terceiros. Troque pelo mesmo arquivo no Modrinth (só no arquivo gerado; o pack não muda) ou tire o mod do pack.',
    trocar: 'Trocar pelo Modrinth',
    trocado: 'Será trocado por {{arquivo}} ({{versao}}) do Modrinth.',
    desfazer: 'Desfazer a troca de {{nome}}',
    semTroca: 'Este arquivo não existe no Modrinth.',
    pagina: 'Página do arquivo',
    desfazerCurto: 'Desfazer troca',
    trocarNome: 'Trocar {{nome}} pelo Modrinth',
  },
  locais: {
    titulo_one: '{{count}} arquivo de terceiros vai dentro do arquivo',
    titulo_other: '{{count}} arquivos de terceiros vão dentro do arquivo',
    texto:
      'Mods, resource packs e shaders que estão direto na pasta do pack seguem junto, e quem joga recebe uma cópia.',
    confirmar: 'Tenho o direito de distribuir estes arquivos',
    confirmarDesc: 'A licença de cada um permite que outras pessoas recebam uma cópia.',
  },
  acao: {
    gerar: 'Gerar instância…',
    dica: 'Você escolhe onde salvar o arquivo .mrpack.',
    bloqueadoMotivo:
      'Resolva os mods bloqueados acima (Trocar pelo Modrinth) ou confirme os arquivos de terceiros para gerar.',
    somenteLeitura: 'Este pack é somente leitura, mas a instância pode ser gerada mesmo assim.',
    falha: 'Não foi possível gerar a instância pronta para o Prism.',
  },
  andamento: {
    rotulo: 'Gerando a instância pronta para o Prism',
    meta: 'Conferindo e registrando cada arquivo',
    aguardando: 'Aguardando outra operação neste pack',
    cancelar: 'Cancelar',
  },
  resultado: {
    titulo: 'Instância pronta para o Prism gerada',
    resumo:
      '{{arquivos}} para o Prism baixar e {{overrides}} dentro do arquivo ({{tamanho}}), salvo em',
    sha: 'SHA-256',
    copiarSha: 'Copiar o SHA-256',
    conferido: 'Conferido.',
    conferidoTexto:
      'O arquivo passou na conferência do formato .mrpack e nas regras do Prism: caminhos seguros, endereços https, conferência e tamanho de cada arquivo.',
    mostrar: 'Mostrar arquivo',
    fechar: 'Fechar',
  },
  passos: {
    titulo: 'Como o jogador instala (Prism Launcher)',
    rotulo: 'Passos da instalação pela instância pronta',
    um: 'Baixe o arquivo e arraste para a janela do Prism Launcher.',
    dois: 'O Prism baixa os mods sozinho. Ele vai pedir para confirmar os mods que não vêm do Modrinth: pode confirmar.',
    tres: 'Use {{memoria}} de memória em Editar instância → Configurações → Memória.',
    memoriaPadrao: '6 GB',
    atualizar:
      'A instância pronta não se atualiza sozinha: a cada versão nova, baixe e arraste o arquivo novo. Para receber as atualizações ao abrir o jogo, use o passo a passo do link do pack.',
  },
};
