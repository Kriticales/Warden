/**
 * Textos da moldura do app: barra de título, barra do app, rodapé e atalhos de navegação (ESTRUTURA; HANDOFF §7).
 */
export const navegacao = {
  pularParaConteudo: 'Pular para o conteúdo',
  marca: 'Warden',
  janela: {
    grupo: 'Janela',
    minimizar: 'Minimizar',
    maximizar: 'Maximizar',
    restaurar: 'Restaurar',
    fechar: 'Fechar',
  },
  configuracoes: 'Configurações',
  meusPacks: 'Meus packs',
  naoEncontrada: {
    titulo: 'Página não encontrada',
    subtitulo: 'Esta página não existe',
    texto: 'O endereço pode ser de uma versão antiga do Warden. Volte para o início e siga daí.',
    voltar: 'Voltar para o início',
  },
  rodape: {
    versao: 'Warden {{version}}',
    versaoDesconhecida: 'Warden',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    navegacao: typeof navegacao;
  }
}
