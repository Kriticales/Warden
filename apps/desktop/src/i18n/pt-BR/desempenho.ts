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
