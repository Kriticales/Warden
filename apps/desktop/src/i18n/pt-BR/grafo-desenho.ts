/**
 * Textos do modo Grafo da seção Mods (SPEC T06, "Ver como: Lista · Grafo"; CA-T06-07; D-11):
 * o grafo focado, a explicação em palavras, a lista das ligações e "Ver o pack inteiro".
 * Termos do glossário (QUALITY §8.2: "Ver como: Grafo", "quem precisa de quem") e do protótipo
 * aprovado (`design/prototipo-final/screens-pack.js`, `graphContent`). Os nomes dos tipos de
 * ligação ("obrigatória", "opcional"…) são os do namespace `grafo` (D-07).
 */
export const grafoDesenho = {
  ferramentas: {
    centro: 'Mod no centro',
    buscarCentro: 'Buscar mod para o centro',
    semResultado: 'Nenhum mod com esse nome',
    profundidade: 'Profundidade',
    nivel_one: '{{count}} nível de cada lado',
    nivel_other: '{{count}} níveis de cada lado',
  },
  carregando: 'Lendo quem precisa de quem…',
  semMods: {
    titulo: 'Nenhum mod para mostrar',
    texto: 'O grafo mostra as dependências entre os mods. Adicione mods ao pack para vê-las aqui.',
  },
  colunas: {
    precisaDe: 'Precisa de',
    centro: 'Mod no centro',
    usadoPor: 'Usado por',
    nivel2Esquerda: 'Que precisam de',
    nivel2Direita: 'Usados por estes',
  },
  regiao: 'Grafo de dependências de {{nome}}',
  no: {
    noPack: 'no pack',
    falta: 'não está no pack',
    biblioteca: 'biblioteca',
    embutidas_one: '{{count}} dentro',
    embutidas_other: '{{count}} dentro',
    dentroDele: 'Dentro dele: {{nomes}}',
    mais_one: 'e mais {{count}} (na lista em texto)',
    mais_other: 'e mais {{count}} (na lista em texto)',
  },
  legenda: {
    titulo: 'Tipos de ligação',
    obrigatoria: 'Obrigatória',
    opcional: 'Opcional ou recomendada',
    incompativel: 'Incompatível',
    inferida: 'Inferida (não declarada pelo mod)',
    embutida: 'Embutida: fica dentro do nó',
  },
  palavras: {
    titulo: 'Em palavras',
    precisaDe: '{{nome}} precisa de:',
    usaSeTiver: 'Usa se estiver no pack, mas não precisa:',
    incompativelCom: 'Incompatível com:',
    dentro: 'Vêm dentro do próprio {{nome}}:',
    precisamDele: 'Precisam do {{nome}}:',
    nenhum: 'nenhum mod do pack',
    seRemover_one: 'Se você remover o {{nome}}, {{count}} mod para de funcionar: {{nomes}}',
    seRemover_other: 'Se você remover o {{nome}}, {{count}} mods param de funcionar: {{nomes}}',
    seRemoverNada: 'Se você remover o {{nome}}, nenhum mod para de funcionar.',
    porQue: 'Por que está no pack:',
  },
  ligacoes: {
    titulo_one: '{{count}} ligação',
    titulo_other: '{{count}} ligações',
    nenhuma: 'Este mod não tem ligações com outros mods do pack.',
    required: '{{de}} precisa de {{para}}',
    optional: '{{de}} usa {{para}} se estiver no pack',
    recommended: '{{de}} recomenda {{para}}',
    suggested: '{{de}} sugere {{para}}',
    breaks: '{{de}} é incompatível com {{para}}',
    conflicts: '{{de}} entra em conflito com {{para}}',
    inferred: '{{de}} parece precisar de {{para}}',
    dentroDe: 'dentro do {{nome}}',
    falta: 'não está no pack',
  },
  orfas: {
    titulo_one: '{{count}} biblioteca sem uso: {{nomes}}.',
    titulo_other: '{{count}} bibliotecas sem uso: {{nomes}}.',
    texto: 'Nenhum mod do pack precisa mais delas. Podem ser removidas.',
    ver: 'Ver {{nome}}',
  },
  rodape:
    'Clique num mod para abrir os detalhes. Para colocar outro mod no centro, use "Ver no grafo" nos detalhes ou o seletor acima.',
  inteiro: {
    abrir: 'Ver o pack inteiro',
    titulo: 'O pack inteiro',
    resumo_one: '{{count}} mod e {{ligacoes}}.',
    resumo_other: '{{count}} mods e {{ligacoes}}.',
    ligacoes_one: '{{count}} ligação',
    ligacoes_other: '{{count}} ligações',
    muitos:
      'Este pack tem {{count}} mods. Com mais de 150, o desenho fica difícil de ler; a lista em texto ao lado mostra as mesmas ligações.',
    desenharAssim: 'Desenhar assim mesmo',
    desenhando: 'Desenhando o pack…',
    falhou: 'Não foi possível desenhar o pack. A lista em texto continua valendo.',
    bibliotecas: 'Bibliotecas',
    regiaoDesenho: 'Desenho do pack inteiro (a mesma informação está na lista em texto)',
    lista: 'Lista em texto',
    precisaDe: 'precisa de {{nomes}}',
    naoPrecisa: 'não precisa de outro mod',
  },
  verNoGrafo: 'Ver no grafo',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    grafoDesenho: typeof grafoDesenho;
  }
}
