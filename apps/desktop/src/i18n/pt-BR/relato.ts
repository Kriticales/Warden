/**
 * "Relatar problema" (UI-02 e UI-03): o botão da barra de título (no Linux, do rodapé), o
 * atalho e o diálogo que monta o relato e o envia direto ao GitHub (com o login) ou abre a
 * issue já preenchida no navegador (sem ele).
 */
export const relato = {
  botao: 'Relatar problema',
  dica: 'Monta um relato para o GitHub com o que aconteceu. Atalho: {{atalho}}',
  atalho: 'Ctrl+Shift+E',
  titulo: 'Relatar problema',
  descricao:
    'O relato vira uma issue no repositório do Warden no GitHub. Confira a pré-visualização antes de enviar.',
  publico: 'Este repositório é público: o relato fica visível para qualquer pessoa.',
  campos: {
    categoria: 'Categoria',
    categorias: {
      error: 'Erro',
      suggestion: 'Sugestão',
      visualOrText: 'Visual ou texto',
      slowness: 'Lentidão',
      other: 'Outro',
    },
    titulo: 'Título',
    tituloDica: 'Uma frase com o problema. Exemplo: "O Testar fecha sem abrir o jogo".',
    tituloObrigatorio: 'Escreva um título para enviar o relato.',
    descricao: 'Descrição',
    descricaoPlaceholder: 'O que você fez, o que aconteceu e o que esperava.',
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
  prints: {
    titulo: 'Prints',
    dica: 'Cole com Ctrl+V, arraste sobre esta janela ou escolha imagens PNG ou JPEG: até 10 MB cada e 5 por relato.',
    aviso:
      'Os prints vão para o repositório do Warden e ficam visíveis como o relato. Confira se não mostram dados pessoais.',
    escolher: 'Escolher imagens…',
    lendo: 'Lendo a imagem…',
    remover: 'Remover',
    removerNome: 'Remover {{nome}}',
    miniatura: 'Print {{n}}: {{nome}}',
    acrescentados_one: '{{count}} print acrescentado.',
    acrescentados_other: '{{count}} prints acrescentados.',
    removido: '{{nome}} removido.',
    recusados: 'Não entraram no relato:',
  },
  semLogin: {
    texto: 'Entre com o GitHub nas Configurações para enviar direto, com prints.',
    ir: 'Ir para Configurações',
  },
  acoes: {
    cancelar: 'Cancelar',
    abrir: 'Abrir no GitHub',
    abrindo: 'Abrindo…',
    enviar: 'Enviar',
    enviando: 'Enviando…',
    verNoGithub: 'Ver no GitHub',
    concluir: 'Concluir',
  },
  etapas: {
    labels: 'Preparando as labels…',
    prints: 'Enviando os prints…',
    issue: 'Criando a issue…',
  },
  progresso: {
    labels: 'Preparando as labels da issue…',
    prints: 'Enviando print {{feito}} de {{total}}…',
    issue: 'Criando a issue…',
  },
  resultado: {
    aberto: 'Relato aberto no navegador',
    abertoTexto: 'Confira o texto no GitHub e clique em "Submit new issue" para enviar.',
    copiado:
      'O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
    naoCopiou:
      'Não deu para copiar o relato completo. A issue abriu com a parte que coube no endereço.',
    falhou: 'O navegador não abriu',
    enviado: 'Relato enviado (#{{numero}})',
    enviadoTexto: 'A issue foi criada no repositório do Warden, com as labels e os prints.',
    naoEnviado: 'O relato não foi enviado',
    rascunhoGuardado: 'O rascunho continua aqui: tente de novo ou use Abrir no GitHub.',
    linkFalhou: 'O navegador não abriu a issue',
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
