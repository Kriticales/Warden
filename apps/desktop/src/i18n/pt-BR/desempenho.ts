/**
 * Desempenho do jogo no Testar (SPEC T13; ARCHITECTURE §7.7; ROADMAP L-10): a faixa acima do
 * console com a RAM do processo e os tempos de carregamento, e o mesmo resumo no resultado
 * ("Abriu em …" com a diferença para a última versão testada).
 */
export const desempenho = {
  faixa: {
    rotulo: 'Desempenho do jogo',
    ram: 'RAM do processo',
    ramSub: 'pico de {{pico}}',
    medindo: 'medindo…',
    abriuEm: 'Abriu em',
    carregando: 'carregando…',
    mundo: 'Entrou no mundo em',
    aindaNao: 'ainda não entrou num mundo',
  },
  resultado: {
    rotulo: 'Desempenho do teste',
    ramPico: 'Pico de RAM do processo',
    ramSub: 'o jogo e os processos que ele abriu',
    semMedida: 'o jogo fechou antes da primeira medição',
    naoCarregou: 'o jogo fechou antes de carregar',
    travouAntes: 'o jogo travou antes de carregar',
    semMundo: 'não entrou num mundo',
  },
  comparacao: {
    mais: '{{diferenca}} a mais que na {{versao}}',
    menos: '{{diferenca}} a menos que na {{versao}}',
    igual: 'o mesmo tempo da {{versao}}',
  },
  /**
   * "Desempenho do último teste" na seção Problemas (SPEC T14; esboço aprovado do bug-01): os
   * números do último teste que carregou, o aviso de versão mais pesada (critério da T33) e os
   * mods que mais usaram processamento no perfil do spark.
   */
  ultimoTeste: {
    titulo: 'Desempenho do último teste',
    carregando: 'Lendo o último teste…',
    erro: 'Não foi possível ler o desempenho do último teste.',
    vazio: 'Ainda não há teste que chegou a carregar.',
    vazioTexto: 'Os números aparecem depois de um teste em que o jogo abriu.',
    abrir: 'Tempo para abrir',
    mundo: 'Até entrar no mundo',
    semMundo: 'não entrou',
    memoria: 'Memória máxima',
    memoriaDe: '{{usado}} de {{maximo}}',
    noLimite: 'no limite',
    ramProcesso: 'RAM do processo',
    tick: 'Tempo por tick (spark)',
    tickValor: '{{ms}} ms',
    /** A variação do tempo para abrir em relação à última versão testada. */
    variacao: '{{sinal}}{{percentual}}% que na {{versao}}',
    variacaoIgual: 'igual à {{versao}}',
    /** Critério de "mais pesada" da T33, com os números. */
    maisPesada: {
      loadTime:
        'A versão {{versao}} abre {{percentual}}% mais lenta que a {{anterior}} ({{agora}} contra {{antes}}), neste computador e com o mesmo perfil de teste.',
      memory:
        'A versão {{versao}} usa {{percentual}}% mais memória que a {{anterior}} ({{agora}} contra {{antes}}), neste computador e com o mesmo perfil de teste.',
      tick: 'A versão {{versao}} tem ticks {{percentual}}% mais lentos que a {{anterior}} ({{agora}} contra {{antes}}), neste computador e com o mesmo perfil de teste.',
    },
    poucosTestes: 'Poucos testes para comparar com a {{anterior}}. Teste de novo para confirmar.',
    sparkTitulo: 'Mods que mais usaram processamento (spark)',
    semSpark: 'Sem perfil do spark neste teste.',
    verTeste: 'Ver o resultado do teste',
    perguntar: 'Perguntar ao Sculk por que ficou mais lento',
    /** O assunto que vai para "Contar algo" na conversa nova. */
    assunto: 'Por que o pack ficou mais lento? {{aviso}}',
    assuntoSemAviso: 'O último teste abriu em {{abrir}}. Quero entender o que mais pesa no pack.',
  },
  /** Valores com a unidade em separado (a unidade sai menor). */
  unidade: {
    mb: 'MB',
    gb: 'GB',
  },
  menosDeUmSegundo: 'menos de 1 s',
  /** Sem valor. */
  vazio: '—',
};

declare module '../catalogo' {
  interface Catalogo {
    desempenho: typeof desempenho;
  }
}
