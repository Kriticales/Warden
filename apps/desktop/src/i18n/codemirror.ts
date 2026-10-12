/**
 * Frases do painel de busca e substituição do editor (CodeMirror), por texto original em
 * inglês. Ficam fora de `pt-BR/` (que vira namespaces sozinho) porque as chaves são as frases do
 * próprio CodeMirror.
 */
export const frasesDoEditor: Record<string, string> = {
  Find: 'Buscar',
  Replace: 'Substituir',
  next: 'próximo',
  previous: 'anterior',
  all: 'todos',
  'match case': 'diferenciar maiúsculas',
  regexp: 'expressão regular',
  'by word': 'palavra inteira',
  replace: 'substituir',
  'replace all': 'substituir todos',
  close: 'fechar',
  'current match': 'ocorrência atual',
  'on line': 'na linha',
  'replaced match on line $': 'ocorrência substituída na linha $',
  'replaced $ matches': '$ ocorrências substituídas',
  'Go to line': 'Ir para a linha',
  go: 'ir',
  selection: 'seleção',
  'Folded lines': 'Linhas dobradas',
  'Unfolded lines': 'Linhas abertas',
  to: 'até',
  'folded code': 'trecho dobrado',
  unfold: 'abrir',
  'Fold line': 'Dobrar linha',
  'Unfold line': 'Abrir linha',
};
