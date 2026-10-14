/**
 * Memória do jogo no Testar (SPEC T13; ARCHITECTURE §7.7; ROADMAP L-11): "Memória do jogo" e
 * "Coletas de memória" na faixa de desempenho, os avisos abaixo dela e o bloco "Memória no
 * último teste" da seção Problemas. Os títulos e as explicações dos achados (`W_LOW_HEAP`,
 * `W_LONG_GC`, `I_HEAP_OVERSIZED`) ficam no catálogo `diagnostico`, com os outros achados.
 */
export const memoria = {
  faixa: {
    memoria: 'Memória do jogo',
    /** "3,1 GB de 6 GB": a unidade e o máximo saem menores. */
    deMaximo: '{{unidade}} de {{maximo}}',
    medindo: 'medindo…',
    semLeitura: 'sem leitura',
    naUltimaColeta: 'na última coleta',
    coletas: 'Coletas de memória',
    pausas: 'Pausas de coleta',
    paradoEmColetas: '{{tempo}} parado em coletas',
    paradoEmPausas: '{{tempo}} parado em pausas',
    grafico: 'Memória nos últimos 5 minutos: entre {{menor}} e {{maior}}',
    graficoIgual: 'Memória nos últimos 5 minutos: {{valor}}',
  },
  motivo: {
    argumento: 'Os argumentos do teste têm {{argumento}}.',
    naoPublica: 'O Java deste teste não publica a memória do jogo.',
  },
  aviso: {
    cheiaTitulo: 'O jogo está quase sem memória.',
    cheiaTexto:
      'Depois de limpar a memória, ele continua usando mais de 90% dos {{maximo}} há {{duracao}}. Aumente para {{sugerida}} em Ajustes do teste.',
    semLeituraTitulo: 'A memória do jogo não pode ser lida neste teste.',
    semLeituraArgumento:
      'O argumento {{argumento}} desliga a leitura. Remova-o em Ajustes do teste. A RAM do processo continua aparecendo.',
    semLeituraAikar:
      'O argumento {{argumento}}, comum nas “flags do Aikar”, desliga a leitura. Remova-o em Ajustes do teste. A RAM do processo continua aparecendo.',
    semLeituraJava:
      'O Java deste teste não publica a memória do jogo. A RAM do processo continua aparecendo.',
    ajustes: 'Ajustes do teste…',
  },
  problemas: {
    titulo: 'Memória no último teste',
    sub: 'Teste de {{quando}}. Os avisos valem até o próximo teste.',
  },
  /** Tempo parado em coletas: "1,2 s", "18,4 s", "2 min 5 s". */
  segundos: '{{valor}} s',
};

declare module '../catalogo' {
  interface Catalogo {
    memoria: typeof memoria;
  }
}
