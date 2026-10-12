/**
 * "Relatar problema" (UI-02; redesenho da UI-03): o botão da barra de título (no Linux, do
 * rodapé), o atalho e o diálogo que monta a issue do GitHub já preenchida.
 */
export const relato = {
  botao: 'Relatar problema',
  dica: 'Monta um relato para o GitHub com o que aconteceu. Atalho: {{atalho}}',
  atalho: 'Ctrl+Shift+E',
  atalhoRotulo: 'Atalho:',
  titulo: 'Relatar problema',
  descricao: 'Vira uma issue no GitHub já preenchida. Lá você cola os prints e envia.',
  publico: 'O repositório é público: o relato fica visível para qualquer pessoa.',
  categoria: {
    legenda: 'Categoria',
    error: {
      nome: 'Erro',
      desc: 'Algo quebrou ou deu erro',
      dica: 'Uma frase com o problema. Exemplo: "O Testar fecha sem abrir o jogo".',
      exemplo: 'O que você fez, o que apareceu na tela e o que esperava que acontecesse.',
    },
    visual: {
      nome: 'Visual ou texto',
      desc: 'Aparência, texto errado',
      dica: 'Onde está e o que está estranho. Exemplo: "Botão cortado na tela de Exportar".',
      exemplo: 'Em que tela está, o que aparece e como deveria aparecer.',
    },
    slowness: {
      nome: 'Lentidão',
      desc: 'Demora ou trava',
      dica: 'O que ficou lento. Exemplo: "Abrir um pack com 300 mods demora 1 minuto".',
      exemplo: 'O que você fez, quanto tempo levou e o tamanho do pack.',
    },
    suggestion: {
      nome: 'Sugestão',
      desc: 'Ideia ou melhoria',
      dica: 'A ideia numa frase. Exemplo: "Filtrar os mods por lado (cliente ou servidor)".',
      exemplo: 'O que você gostaria que o Warden fizesse e por quê.',
    },
    other: {
      nome: 'Outro',
      desc: 'Qualquer outra coisa',
      dica: 'Uma frase com o assunto.',
      exemplo: 'Conte o que quiser.',
    },
  },
  campos: {
    titulo: 'Título',
    tituloObrigatorio: 'Escreva um título para abrir o relato.',
    descricao: 'Descrição',
    opcional: '(opcional)',
    contador: '{{count}} / {{max}}',
    contadorLeitor: '{{count}} de {{max}} caracteres',
    incluirRegistros: 'Incluir os registros do app',
    incluirRegistrosDica: 'As últimas linhas dos registros do Warden, que ajudam a achar a causa.',
    print: 'Copiar um print desta tela',
    printDica: 'O Warden copia um print da janela antes de abrir o GitHub. Na issue, é só Ctrl+V.',
    novo: 'novo',
  },
  passos: {
    titulo: 'Como fica o envio',
    abrir: 'Abrir no GitHub',
    abrirTexto: 'A issue abre no navegador com tudo preenchido.',
    colar: 'Colar os prints',
    colarTexto: 'Ctrl+V na caixa de texto da issue.',
    enviar: 'Submit new issue',
    enviarTexto: 'O relato chega para o orquestrador na hora.',
  },
  coletado: {
    titulo: 'O que o Warden coleta sozinho',
    registrosRotulo: 'Registros',
    registros_one: '{{count}} linha, com pastas e chaves trocadas por <usuário> e <segredo>',
    registros_other: '{{count}} linhas, com pastas e chaves trocadas por <usuário> e <segredo>',
    semRegistros: 'não incluídos',
  },
  previa: {
    titulo: 'Ver como a issue vai ficar',
    explicacao:
      'Exatamente o que vai para o GitHub. Pastas pessoais, nomes e chaves já foram trocados por marcadores como <usuário> e <segredo>.',
    tituloDaIssue: 'Título da issue',
    rotulos: 'Rótulos',
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
    registroComPrint:
      'Os registros são longos demais para o endereço da issue: só as linhas mais novas entram. O print vai para a área de transferência; depois de colar o print, o aviso tem o botão "Copiar relato completo" para mandar tudo.',
    textoComPrint:
      'O relato é longo demais para o endereço da issue e foi cortado no fim. O print vai para a área de transferência; depois de colar o print, o aviso tem o botão "Copiar relato completo" para colar o texto inteiro.',
  },
  acoes: {
    cancelar: 'Cancelar',
    abrir: 'Abrir no GitHub',
    abrindo: 'Abrindo…',
    copiandoPrint: 'Copiando o print…',
  },
  resultado: {
    aberto: 'Relato aberto no navegador',
    abertoTexto: 'Cole os prints (Ctrl+V) e clique em "Submit new issue".',
    printCopiado: 'Cole o print (Ctrl+V) na issue e clique em "Submit new issue".',
    printCopiadoComCorte:
      'Cole o print (Ctrl+V) na issue. Depois clique em "Copiar relato completo" e cole o texto no lugar do que foi cortado.',
    printFalhou:
      'O print não foi copiado. Se quiser, tire o print você mesmo e cole na issue (Ctrl+V).',
    printFalhouComCorte:
      'O print não foi copiado. O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
    copiado:
      'O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
    naoCopiou:
      'Não deu para copiar o relato completo sozinho. Clique em "Copiar relato completo" para tentar de novo.',
    copiarRelato: 'Copiar relato completo',
    copiarRelatoAlt:
      'O relato completo não está na área de transferência. Aperte F8 para ir ao aviso e use "Copiar relato completo".',
    relatoCopiado: 'Relato completo copiado',
    relatoCopiadoTexto: 'Cole na issue (Ctrl+V) no lugar do texto cortado.',
    relatoNaoCopiado: 'Não deu para copiar o relato completo',
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
