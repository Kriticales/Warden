/**
 * Buscar em todas as configs (SPEC T12; C-05; protótipo `configs-busca`): o campo acima da
 * árvore e os resultados agrupados por arquivo. O rótulo traduzido do mod e o filtro "Só o que
 * mudou do padrão" entram com a C-04 e a C-06. Glossário: QUALITY §8.2.
 */
export const configsBusca = {
  campo: 'Buscar em todas as configs',
  limpar: 'Limpar busca',
  titulo: '{{resultados}} {{arquivos}} para “{{busca}}”',
  resultados_one: '{{count}} resultado',
  resultados_other: '{{count}} resultados',
  arquivos_one: 'em {{count}} arquivo',
  arquivos_other: 'em {{count}} arquivos',
  ajuda:
    'Procura no nome da chave, no comentário e no valor, sem diferenciar maiúsculas nem acentos. Clique para abrir o arquivo já na chave.',
  buscando: 'Buscando…',
  lendo:
    'Lendo as configs: {{lidos}} de {{total}} arquivos. Os resultados aparecem enquanto a leitura continua.',
  nada: 'Nenhuma config com “{{busca}}”.',
  nadaTexto:
    'Confira a grafia ou use menos palavras: cada palavra precisa aparecer na mesma chave, no comentário ou no valor dela.',
  semInstancia:
    'A instância de teste ainda não existe. Teste o pack uma vez para buscar nas configs dela.',
  cortado:
    'Mostrando os primeiros {{mostrados}} resultados. Use mais palavras ou o filtro por mod para ver o resto.',
  maisNoArquivo_one: 'Mais {{count}} resultado neste arquivo',
  maisNoArquivo_other: 'Mais {{count}} resultados neste arquivo',
  linha: 'linha {{linha}}',
  erro: 'Não foi possível buscar nas configs.',
  filtroMod: {
    rotulo: 'Só deste mod',
    todos: 'Todos os mods',
    mod: 'Só do mod {{mod}}',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    configsBusca: typeof configsBusca;
  }
}
