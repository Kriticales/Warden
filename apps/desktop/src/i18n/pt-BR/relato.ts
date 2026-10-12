/**
 * "Relatar problema" (UI-02): o botão da barra de título (no Linux, do rodapé), o atalho e o
 * diálogo que monta a issue do GitHub já preenchida.
 */
export const relato = {
  botao: 'Relatar problema',
  dica: 'Monta um relato para o GitHub com o que aconteceu. Atalho: {{atalho}}',
  atalho: 'Ctrl+Shift+E',
  titulo: 'Relatar problema',
  descricao:
    'O relato vira uma issue do GitHub já preenchida. Você confere tudo no navegador antes de enviar.',
  publico: 'Este repositório é público: o relato fica visível para qualquer pessoa.',
  campos: {
    titulo: 'Título',
    tituloDica: 'Uma frase com o problema. Exemplo: "O Testar fecha sem abrir o jogo".',
    tituloObrigatorio: 'Escreva um título para abrir o relato.',
    oQueFez: 'O que você fez',
    oQueFezPlaceholder: 'Os passos até o problema, na ordem.',
    oQueAconteceu: 'O que aconteceu',
    oQueAconteceuPlaceholder: 'O que apareceu na tela, mensagens de erro.',
    oQueEsperava: 'O que você esperava',
    oQueEsperavaPlaceholder: 'O que deveria ter acontecido.',
    opcional: '(opcional)',
    incluirRegistros: 'Incluir os registros do app',
    incluirRegistrosDica: 'As últimas linhas dos registros do Warden, que ajudam a achar a causa.',
  },
  coletado: {
    titulo: 'Coletado pelo Warden',
    registros_one: 'Registros: {{count}} linha',
    registros_other: 'Registros: {{count}} linhas',
    semRegistros: 'Registros: não incluídos',
  },
  previa: {
    titulo: 'Pré-visualização',
    explicacao:
      'Exatamente o que vai para o GitHub. Pastas pessoais, nomes e chaves já foram trocados por marcadores como <usuário> e <segredo>.',
    tituloDaIssue: 'Título da issue',
    semTitulo: '(sem título)',
    corpo: 'Texto da issue',
    montando: 'Montando a pré-visualização…',
    erro: 'A pré-visualização não foi montada',
  },
  corte: {
    registro:
      'Os registros são longos demais para o endereço da issue: só as linhas mais novas entram. Ao abrir, o relato completo vai para a área de transferência; cole na issue se quiser mandar tudo.',
    texto:
      'O relato é longo demais para o endereço da issue e foi cortado no fim. Ao abrir, o relato completo vai para a área de transferência; cole na issue no lugar do texto cortado.',
  },
  print: 'Para mandar um print, cole a imagem (Ctrl+V) na caixa de texto da issue, no GitHub.',
  acoes: {
    cancelar: 'Cancelar',
    abrir: 'Abrir no GitHub',
    abrindo: 'Abrindo…',
  },
  resultado: {
    aberto: 'Relato aberto no navegador',
    abertoTexto: 'Confira o texto no GitHub e clique em "Submit new issue" para enviar.',
    copiado:
      'O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
    naoCopiou:
      'Não deu para copiar o relato completo. A issue abriu com a parte que coube no endereço.',
    falhou: 'O navegador não abriu',
  },
  telas: {
    desconhecida: 'Tela não identificada',
    primeiraExecucao: 'Primeira execução',
    configuracoes: 'Configurações',
    meusPacks: 'Meus packs',
    criarPack: 'Meus packs › Criar pack',
    abrirPack: 'Meus packs › Abrir ou importar',
    pack: 'Pack',
    mods: 'Pack › Mods',
    adicionar: 'Pack › Adicionar ao pack',
    historico: 'Pack › Histórico',
    exportar: 'Pack › Exportar',
    testar: 'Pack › Testar',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    relato: typeof relato;
  }
}
