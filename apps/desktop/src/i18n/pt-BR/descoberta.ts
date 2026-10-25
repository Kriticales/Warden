/**
 * Descoberta (SPEC T08, P1-16; ADR-0068): início com populares e atualizados, categorias e as
 * partes da pré-visualização completa (galeria, versões com notas, dependências). Esboço
 * aprovado: `design/bug-01/adicionar.html` (opção 4). Glossário: QUALITY §8.2.
 */
export const descoberta = {
  inicio: {
    rotulo: 'Início da descoberta',
    populares: 'Populares',
    atualizados: 'Atualizados recentemente',
    verTodos: 'Ver todos',
    verTodosPopulares: 'Ver todos os populares',
    verTodosAtualizados: 'Ver todos os atualizados',
    vazio: 'Nada para mostrar aqui ainda.',
    vazioTexto: 'Nenhum item popular funciona em {{alvo}}. Digite um nome para buscar.',
    erro: 'Não foi possível montar o início agora.',
  },
  categorias: {
    titulo: 'Categoria',
    todas: 'Todas',
    mais_one: 'Mais {{count}} categoria',
    mais_other: 'Mais {{count}} categorias',
    menos: 'Mostrar menos',
    carregando: 'Lendo as categorias…',
    erro: 'Não foi possível ler as categorias.',
  },
  indice: {
    rotulo: 'Partes desta página',
    descricao: 'Descrição',
    galeria: 'Galeria',
    versoes: 'Versões',
    dependencias: 'Dependências',
    links: 'Links',
  },
  galeria: {
    titulo: 'Galeria',
    carregando: 'Lendo as imagens…',
    erro: 'Não foi possível ler a galeria.',
    vazia: 'O autor não publicou imagens.',
    lista: 'Imagens do projeto',
    abrir: 'Ver {{name}} em tamanho grande',
    semTitulo: 'Imagem {{n}}',
    visualizador: 'Imagem {{n}} de {{total}}',
    anterior: 'Imagem anterior',
    proxima: 'Próxima imagem',
    naoCarregou: 'Esta imagem não carregou.',
  },
  versoes: {
    titulo: 'Versões para {{alvo}}',
    lista: 'Versões compatíveis',
    nenhuma: 'Nenhuma versão deste projeto funciona no pack.',
    maisNova: 'mais nova',
    selecionada: 'a que vai entrar',
    publicada: 'publicada {{when}}',
    canal: {
      release: 'Estável',
      beta: 'Beta',
      alpha: 'Alpha',
    },
    notasCarregando: 'Lendo as notas desta versão…',
    notasErro: 'Não foi possível ler as notas desta versão.',
    semNotas: 'O autor não escreveu notas para esta versão.',
    usar: 'Usar esta versão',
  },
  dependencias: {
    titulo: 'Dependências',
    escolhaUmaVersao: 'Escolha uma versão para ver as dependências dela.',
    carregando: 'Lendo as dependências…',
    erro: 'Não foi possível ler as dependências.',
    obrigatoria: 'Obrigatória',
    opcional: 'Opcional',
    incompativel: 'Incompatível',
    nenhuma: 'nenhuma declarada',
    jaNoPack: 'Já no pack',
    seraAdicionada: 'Será adicionada',
    conflito: 'Já está no pack: vai dar conflito',
    opcionalNaoAdicionada: 'não será adicionada sozinha',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    descoberta: typeof descoberta;
  }
}
