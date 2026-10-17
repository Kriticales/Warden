/**
 * Padrões das configs (SPEC T12 e T14, parte P1; C-06): "padrão: X" com a origem, a marca de
 * alterado, **Restaurar padrão** por chave e para o arquivo todo (sempre com a diferença), o
 * filtro "Só o que mudou do padrão" no formulário e na busca, o erro de faixa no próprio campo
 * e a correção **Restaurar padrão** em Problemas.
 */
export const configsPadrao = {
  campo: {
    padrao: 'padrão:',
    origem_specComment: 'do comentário do arquivo (Default:)',
    origem_legacyComment: 'do comentário do arquivo ([default: …])',
    origem_commentPhrase: 'tirado do texto do comentário',
    deduzido: 'deduzido',
    deduzidoDica:
      'Tirado do texto do comentário; pode não ser exato. Por isso a chave não é marcada como alterada nem pode ser restaurada.',
    desconhecido: 'padrão desconhecido',
    desconhecidoDica:
      'O arquivo não informa o padrão desta chave, então o Warden não sabe se ela mudou.',
    alterado: 'alterado',
    alteradoDica: 'O valor é diferente do padrão que o arquivo informa.',
    restaurarChave: 'Restaurar padrão de {{chave}}',
    foraDaFaixa: '{{valor}} está fora da faixa {{faixa}}. O jogo troca por {{padrao}} sem avisar.',
    foraDaFaixaSemPadrao:
      '{{valor}} está fora da faixa {{faixa}}. O jogo troca pelo padrão do mod sem avisar.',
    foraDosValores:
      '{{valor}} não está entre os valores aceitos. O jogo troca por {{padrao}} sem avisar.',
    foraDosValoresSemPadrao:
      '{{valor}} não está entre os valores aceitos. O jogo troca pelo padrão do mod sem avisar.',
  },
  faixa: {
    ambos: '{{min}} a {{max}}',
    min: 'a partir de {{min}}',
    max: 'até {{max}}',
  },
  filtro: {
    rotulo: 'Só o que mudou do padrão',
    dica: 'Só as chaves com valor diferente do padrão que o próprio arquivo informa.',
    nenhumaNoArquivo: 'Nenhuma chave deste arquivo está diferente do padrão que ele informa.',
    contagem_one: '{{count}} chave diferente do padrão',
    contagem_other: '{{count}} chaves diferentes do padrão',
  },
  busca: {
    marca: 'Mudou do padrão',
    titulo: '{{resultados}} em {{arquivos}} mudaram do padrão',
    resultados_one: '{{count}} chave',
    resultados_other: '{{count}} chaves',
    arquivos_one: '{{count}} arquivo',
    arquivos_other: '{{count}} arquivos',
    nada: 'Nenhuma chave mudou do padrão.',
    nadaTexto:
      'Só contam as chaves cujo arquivo informa o padrão (Default: do NeoForge, [default: …] do Forge antigo).',
  },
  menu: {
    rotulo: 'Mais ações do arquivo',
    restaurarTudo: 'Restaurar o arquivo todo para o padrão…',
    restaurarTudoDesc: 'Mostra as diferenças antes de gravar',
    nadaARestaurar: 'Nenhuma chave está diferente do padrão que o arquivo informa.',
  },
  dialogo: {
    tituloChave: 'Restaurar o padrão de {{chave}}?',
    tituloArquivo: 'Restaurar {{arquivo}} para o padrão?',
    sub: 'Confira as diferenças. A mudança vai para o editor; salve para gravar no arquivo, ou descarte para voltar atrás.',
    chaves_one: '{{count}} chave volta ao padrão.',
    chaves_other: '{{count}} chaves voltam ao padrão.',
    calculando: 'Calculando as diferenças…',
    erro: 'Não foi possível restaurar o padrão.',
    cancelar: 'Cancelar',
    confirmar: 'Restaurar',
  },
  problema: {
    titulo: 'Restaurar o padrão de {{chave}}?',
    sub: 'A chave volta ao padrão que o arquivo {{arquivo}} informa. Confira as diferenças antes de gravar.',
    carregando: 'Lendo o arquivo…',
    erro: 'Não foi possível restaurar o padrão.',
    cancelar: 'Cancelar',
    confirmar: 'Restaurar e salvar',
    gravando: 'Salvando…',
    feito: '{{chave}} voltou ao padrão em {{arquivo}}.',
    abrir: 'Abrir em Configs',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    configsPadrao: typeof configsPadrao;
  }
}
