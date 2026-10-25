/**
 * Textos da saúde do pack: o topo da seção Problemas e a coluna Saúde de Meus packs (SPEC T14 e
 * T02; ROADMAP D-08; protótipo `problemas`, `healthPanel`, e `packs`, `packTableHead`). Termos do
 * glossário (QUALITY §8.2): "saúde do pack" com as faixas Ótimo, Bom, Atenção e Crítico (nunca
 * "score" ou "pontuação"), "alteram o mesmo ponto do jogo", "teste", "travou".
 */
export const saude = {
  titulo: 'Saúde do pack',
  rotulo: 'Saúde do pack: {{nota}} de 100, {{faixa}}',
  rotuloSemDados: 'Saúde do pack: sem dados',
  semNota: '—',
  de100: '/100',
  faixa: {
    great: 'Ótimo',
    good: 'Bom',
    attention: 'Atenção',
    critical: 'Crítico',
    semDados: 'Sem dados',
  },
  aviso: 'Resumo dos problemas conhecidos. Não garante que o pack funciona.',
  semDados: 'A saúde aparece depois da primeira verificação do pack.',
  calculando: 'Calculando a saúde do pack…',
  erro: 'Não foi possível calcular a saúde do pack.',
  tirouPontos: 'O que tirou pontos',
  nadaTirou: 'Nada tirou pontos.',
  comoCalcula: 'Como a nota é calculada',
  /** Selo da barra do painel: quando a verificação rodou. */
  verificado: 'Verificado {{quando}}',
  naoVerificado: 'Ainda não verificado',
  pontos: '−{{pontos}}',
  pontosRotulo: 'menos {{pontos}} pontos',
  limite: '(o limite desta categoria)',
  ignorado: '(ignorado por você)',
  faixas:
    'Faixas: {{otimo}} a 100 Ótimo, {{bom}} a {{bomAte}} Bom, {{atencao}} a {{atencaoAte}} Atenção, abaixo de {{atencao}} Crítico. Avisos que você mandar ignorar não tiram pontos.',
  categoria: {
    problems: 'Problemas encontrados:',
    lastTest: 'Último teste:',
    crashes: 'Travamentos recentes:',
    mixins: 'Mods que alteram o mesmo ponto do jogo:',
    maintenance: 'Manutenção:',
    download: 'Download pelos jogadores:',
    performance: 'Desempenho:',
  },
  /** Junta duas partes do resumo ("2 erros e 1 aviso"). */
  e: '{{a}} e {{b}}',
  erros_one: '{{count}} erro',
  erros_other: '{{count}} erros',
  avisos_one: '{{count}} aviso',
  avisos_other: '{{count}} avisos',
  riscoAlto: '{{count}} com risco alto',
  riscoMedio: '{{count}} com risco médio',
  ultimoTeste: {
    crashed: 'o último teste travou.',
    neverTested: 'o pack ainda não foi testado.',
    changedSince: 'o pack mudou depois do último teste.',
  },
  verTeste: 'Ver o teste',
  causas_one: '{{count}} causa nas últimas {{sessoes}} vezes que você testou.',
  causas_other: '{{count}} causas diferentes nas últimas {{sessoes}} vezes que você testou.',
  /** Travamento cuja linha não está mais na lista de Travamentos. */
  travamento: 'Um travamento',
  /** Achado que não está mais no relatório. */
  achado: 'Um problema do pré-teste',
  arquivado: '“{{nome}}” foi arquivado pelo autor',
  alfaOuBeta: '“{{nome}}” está numa versão alfa ou beta',
  semVersaoNova: '“{{nome}}” está sem versão nova há {{meses}} meses',
  download_one: '{{count}} mod da CurseForge precisa ser baixado à mão.',
  download_other: '{{count}} mods da CurseForge precisam ser baixados à mão.',
  nomeEntreAspas: '“{{nome}}”',
  lento:
    'o último teste levou {{agora}} para carregar, {{mais}}% a mais que o último teste bom ({{antes}}).',
  memoriaAbaixo:
    'a memória do último teste ({{memoria}}) ficou abaixo da sugestão para a quantidade de mods ({{sugestao}}).',
  coluna: 'Saúde',
  colunaDica: 'Resumo dos problemas conhecidos, de 0 a 100. Não garante que o pack funciona.',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    saude: typeof saude;
  }
}
