/**
 * Seção Exportar do pack (SPEC T19; protótipo `exportar`): antes de exportar → formato → o que
 * vai → exportar. As frases dos alertas da prévia ficam aqui, uma por `kind` (a higiene usa as
 * frases de motivo de `packs.motivo`, as mesmas de Abrir pack).
 */
export const exportar = {
  titulo: 'Exportar',
  sub: 'Gera o pack para quem vai jogar, só com o necessário. O pack no Warden não muda.',
  carregando: 'Lendo o que vai no pack…',
  erroPrevia: 'Não foi possível ler o que vai no pack.',
  antes: {
    titulo: '1. Antes de exportar',
    higieneOk:
      'Nenhum arquivo que não deveria ir para quem joga (registros, cópias de segurança, caches).',
    higiene_one: '{{count}} arquivo na pasta do pack não deveria ir para quem joga.',
    higiene_other: '{{count}} arquivos na pasta do pack não deveriam ir para quem joga.',
    revisar: 'Revisar e limpar',
    limparTitulo: 'Limpar a pasta do pack',
    erros_one: 'O pack tem {{count}} erro. Veja em Problemas antes de exportar.',
    erros_other: 'O pack tem {{count}} erros. Veja em Problemas antes de exportar.',
    naoSalvas:
      'Há alterações não salvas: a exportação usa o estado de agora, que não é uma versão salva. Recomendamos salvar uma versão antes (Salvar versão, no alto).',
    salvas: 'Sem alterações não salvas: a exportação é igual à última versão salva.',
  },
  formato: {
    titulo: '2. Formato',
    pasta: {
      titulo: 'Pasta packwiz',
      desc: 'Para hospedar em outro lugar. Para o GitHub, use Publicar versão no Histórico.',
    },
    zip: {
      titulo: 'Arquivo .zip do pack packwiz',
      desc: 'A mesma coisa, num arquivo só.',
    },
  },
  conteudo: {
    titulo: '3. O que vai no pack',
    total: '{{arquivos}}, {{tamanho}} no total.',
    arquivos_one: '{{count}} arquivo',
    arquivos_other: '{{count}} arquivos',
    referencias_one: '{{count}} referência',
    referencias_other: '{{count}} referências',
    referenciasNota: '(os .jar não vão: quem joga baixa)',
    controle: '(nome, versões e a lista do que vai)',
    raiz: 'Na raiz do pack',
    referencia: 'Referência',
    local: 'Arquivo local',
    excluirArquivo: 'Excluir do pack: {{path}}',
    excluirPasta: 'Excluir a pasta {{path}} do pack',
    dica: 'Algo aqui não deveria ir? Use Excluir do pack na linha do arquivo (abra a pasta para ver os arquivos).',
  },
  alertas: {
    titulo_one: '{{count}} arquivo pede atenção',
    titulo_other: '{{count}} arquivos pedem atenção',
    largeFile: 'Arquivo grande (mais de 20 MB): quem joga vai baixar tudo isso.',
    optionsOverridesPreferences:
      'Substitui as preferências de quem já joga (teclas, gráficos, som) a cada atualização.',
    looseRootFile: 'Arquivo solto na raiz do pack: confira se ele deveria mesmo ir para quem joga.',
    higiene: 'Parece não ser para quem joga: {{motivo}}',
  },
  excluir: {
    titulo: 'Excluir do pack',
    texto:
      'O Warden acrescenta esta linha ao .packwizignore do pack e atualiza o índice. O arquivo continua na pasta, mas deixa de ir para quem joga.',
    textoPasta:
      'O Warden acrescenta esta linha ao .packwizignore do pack e atualiza o índice. Os arquivos continuam na pasta, mas a pasta inteira deixa de ir para quem joga.',
    desfazer: 'Para desfazer, apague a linha do .packwizignore.',
    confirmar: 'Excluir do pack',
    confirmando: 'Excluindo…',
    feito: 'Excluído do pack: {{path}}',
  },
  acao: {
    exportar: 'Exportar…',
    dicaPasta: 'A seguir, você escolhe uma pasta vazia para o pack.',
    dicaZip: 'A seguir, você escolhe onde salvar o arquivo .zip.',
    somenteLeitura: 'Este pack está só para leitura: dá para exportar, mas não excluir nem limpar.',
  },
  erros: {
    titulo_one: 'O pack tem {{count}} erro.',
    titulo_other: 'O pack tem {{count}} erros.',
    texto: 'Quem jogar pode ter os mesmos problemas. Recomendamos corrigir em Problemas antes.',
    confirmar: 'Exportar mesmo assim',
  },
  andamento: {
    rotulo: 'Exportando',
    meta: 'Copiando os arquivos e conferindo com o packwiz no fim',
    aguardando: 'Aguardando outra operação neste pack',
    cancelar: 'Cancelar',
  },
  falha: 'Não foi possível exportar.',
  resultado: {
    titulo: 'Pack exportado',
    resumo: '{{arquivos}}, {{tamanho}}, em',
    conferido: 'Conferido com o packwiz.',
    conferidoTexto:
      'O packwiz refez o índice na cópia e nada mudou: o pack exportado é exatamente o que ele geraria.',
    fechar: 'Fechar',
    abrirPasta: 'Abrir pasta',
    mostrarArquivo: 'Mostrar o arquivo',
  },
} as const;
