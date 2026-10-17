/**
 * "Testar como o jogador recebe" (SPEC T13; ROADMAP L-07): o item do menu ▾ do Testar, as
 * etapas que o backend manda (`jogador:etapa.*`) e o resultado (o que o jogador recebeu × o
 * pack).
 */
export const jogador = {
  menu: {
    item: 'Testar como o jogador recebe',
    descricao: 'Instala pelo link do pack, como um jogador',
  },
  etapa: {
    exportar: 'Exportando o pack para servir neste computador',
    instalador: 'Baixando o instalador do jogador (uma vez)',
    instalar: 'Instalando do zero pelo link do pack, como um jogador',
  },
  resultado: {
    titulo: 'Como o jogador recebe',
    origemPublicado: 'Instalado do zero pelo link do pack (versão {{versao}} publicada).',
    origemLocal:
      'O pack ainda não foi publicado: instalado do zero pelo pack exportado agora, servido só neste computador.',
    recebidos_one: 'O jogador recebeu {{count}} arquivo.',
    recebidos_other: 'O jogador recebeu {{count}} arquivos.',
    versaoAnterior:
      'O GitHub ainda está entregando a versão anterior. Tente de novo em alguns minutos.',
    tentarDeNovo: 'Testar de novo como o jogador',
    bloqueados_one: '{{count}} mod não baixa sozinho para os jogadores',
    bloqueados_other: '{{count}} mods não baixam sozinhos para os jogadores',
    bloqueadosTexto:
      'O autor não deixa outros apps baixarem esses arquivos. Quem jogar o pack vai precisar baixá-los pelo site da CurseForge.',
    crashAssistantChegou: 'O Crash Assistant (ferramenta do jogador) chegou ao jogador.',
    crashAssistantFaltou:
      'O Crash Assistant (ferramenta do jogador) não chegou ao jogador. Confira se ele está na versão publicada.',
    igual: 'O jogador recebeu o mesmo que o pack.',
    tabela: {
      rotulo: 'Diferenças entre o que o jogador recebeu e o pack',
      item: 'Item',
      arquivo: 'Arquivo',
      oQue: 'O que acontece',
    },
    tipo: {
      blocked: 'Não baixa sozinho: o jogador precisa baixar pelo site.',
      notDownloaded: 'Faltando: o download falhou.',
      notPublished: 'Faltando: ainda não está na versão que os jogadores recebem.',
      serverOnly:
        'Lado errado: na versão que os jogadores recebem, está marcado como "Só servidor".',
      optionalOn: 'Opcional: o jogador escolhe na instalação (vem marcado).',
      optionalOff: 'Opcional: o jogador escolhe na instalação (vem desmarcado).',
      extra: 'A mais: o jogador recebe, mas não está mais no pack.',
    },
    carregando: 'Lendo o resultado…',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    jogador: typeof jogador;
  }
}
