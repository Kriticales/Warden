/**
 * Busca do culpado com o servidor local (SPEC T25 e T27; ROADMAP D-13; protótipo
 * `culpado-config~velho`): a explicação e o pedido no diálogo "Encontrar o mod culpado", o
 * servidor de cada rodada na tela da busca, as rodadas que travaram no servidor e o log dos dois
 * lados. A conta de memória, a EULA e "Mostrando: Servidor · Jogo" são os textos do namespace
 * `servidor` (L-09).
 *
 * Glossário (QUALITY §8): "servidor local", "busca do culpado", "rodada", "mundo de teste".
 */
export const buscaComServidor = {
  pedido: {
    titulo: 'Para este problema, o Warden precisa abrir um servidor neste computador.',
    versao:
      'No Minecraft {{minecraft}} o jogo não entra direto num mundo. Em cada rodada o Warden abre um servidor só para você (127.0.0.1), com uma cópia do mundo de teste que é apagada no fim, e o jogo entra nele. O servidor fecha sozinho ao fim de cada rodada.',
    servidor:
      'Este travamento foi no servidor. Em cada rodada o Warden abre um servidor só para você (127.0.0.1), com uma cópia do mundo de teste que é apagada no fim, e o jogo entra nele. O servidor fecha sozinho ao fim de cada rodada.',
    pode: 'Entendi, pode abrir',
    aceito: 'Combinado: cada rodada abre o servidor local.',
    naoAbrir: 'Não abrir o servidor',
    semAceite:
      'Sem o servidor, a busca usa o modo em que você diz se aconteceu: em cada rodada você entra no mundo e responde.',
    pouca:
      'Com {{livre}} livres, o computador pode ficar lento ou o jogo pode fechar durante a busca. Feche outros programas ou use um perfil do teste com menos memória.',
    soCliente_one: '{{count}} mod só de cliente fica fora do servidor.',
    soCliente_other: '{{count}} mods só de cliente ficam fora do servidor.',
  },
  sintoma: {
    servidor:
      'No Minecraft {{minecraft}} o jogo não entra sozinho num mundo: em cada rodada o Warden abre um servidor neste computador, depois de pedir.',
  },
  agora: {
    preparando: 'Servidor local: preparando a pasta da rodada',
    abrindo: 'Servidor local: abrindo em 127.0.0.1, porta {{porta}}',
    pronto: 'Servidor local pronto em 127.0.0.1, porta {{porta}}; o jogo entra nele.',
    parando: 'Servidor local: fechando ao fim da rodada',
  },
  rodada: {
    noServidor: 'no servidor',
  },
  resultado: {
    titulo: 'Com o servidor local',
    texto_one:
      'Cada rodada abriu um servidor neste computador e o jogo entrou nele. {{count}} rodada travou no servidor; o log de cada rodada tem os dois lados.',
    texto_other:
      'Cada rodada abriu um servidor neste computador e o jogo entrou nele. {{count}} rodadas travaram no servidor; o log de cada rodada tem os dois lados.',
    semFalha:
      'Cada rodada abriu um servidor neste computador e o jogo entrou nele. Nenhuma rodada travou no servidor; o log de cada rodada tem os dois lados.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    buscaComServidor: typeof buscaComServidor;
  }
}
