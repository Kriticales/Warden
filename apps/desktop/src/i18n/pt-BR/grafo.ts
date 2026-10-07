/**
 * Textos do bloco "Depende de / Usado por / Por que está no pack" dos detalhes do item (T07) e
 * das consultas do grafo (T06; D-07). Termos do glossário (QUALITY §8.2) e do protótipo
 * aprovado (`design/prototipo-final/screens-pack.js`).
 */
export const grafo = {
  titulo: 'Dependências',
  carregando: 'Lendo as dependências…',
  dependeDe: 'Depende de',
  usadoPor: 'Usado por',
  porQueEstaNoPack: 'Por que está no pack',
  seRemover: 'Se você remover este item',
  nenhumModDependeDe: 'Nenhum outro mod',
  nenhumModUsa: 'Nenhum mod do pack',
  tipo: {
    required: 'obrigatória',
    optional: 'opcional',
    recommended: 'recomendada',
    suggested: 'sugerida',
    breaks: 'incompatível',
    conflicts: 'conflito',
    inferred: 'inferida',
  },
  estado: {
    inPack: 'no pack',
    own: 'vem dentro deste mod',
    missing: 'falta no pack',
    breaksInPack: 'está no pack',
    breaksMissing: 'não está no pack',
  },
  fornecidoPor: 'fornecido por {{nome}}',
  fornecidoPorEmbutido: 'dentro de {{nome}}',
  fornecidoComoAlias: 'como {{id}}',
  dentroDesteMod: 'dentro deste mod',
  versoes: 'versões {{faixa}}',
  porque: {
    voce: 'Você adicionou',
    voceEm: 'Você adicionou em {{data}}',
    ninguemExige: 'Você adicionou: nenhum outro item o exige',
    exigidoPor: 'Exigido por {{nome}}',
    queEExigidoPor: 'que é exigido por {{nome}}',
    queVoceAdicionou: 'que você adicionou',
    semUso: 'Nenhum mod usa mais; pode ser removido',
    soOpcional_one: 'Só é usado como opcional por {{nomes}}',
    soOpcional_other: 'Só é usado como opcional por {{nomes}}',
    ciclo: 'Faz parte de um ciclo de dependência com {{nomes}}',
  },
  afetados_one: '{{count}} mod para de funcionar: {{nomes}}',
  afetados_other: '{{count}} mods param de funcionar: {{nomes}}',
  nenhumAfetado: 'Nenhum outro item para de funcionar.',
  erro: 'Não foi possível ler as dependências deste item',
} as const;
