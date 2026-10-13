/**
 * Formulário das configs (SPEC T12, parte P1; C-04): alternância entre formulário e texto,
 * campos com rótulo, chave real, faixa e padrão, trechos só em texto, queda para o texto com o
 * motivo, "Não substituir se o jogador já tiver" e "Copiar para defaultconfigs/".
 */
export const configsFormulario = {
  modo: {
    rotulo: 'Modo do editor',
    formulario: 'Formulário',
    texto: 'Texto',
    indisponivel: 'Este arquivo só pode ser editado como texto.',
  },
  carregando: 'Montando o formulário…',
  erro: 'Não foi possível montar o formulário.',
  area: 'Campos de {{arquivo}}',
  somenteTexto: {
    titulo: 'Este arquivo só pode ser editado como texto.',
    unsupportedFormat:
      'O formulário funciona com TOML, JSON, JSON5, .properties, .cfg do Forge e options.txt.',
    notForgeCfg: 'Este .cfg não é do formato de configs do Forge.',
    tooLarge: 'O arquivo passa de 2 MB.',
    notUtf8: 'O arquivo não está em UTF-8.',
    parseFailed:
      'O texto não segue o formato na linha {{line}}, coluna {{column}}. Corrija no texto.',
    notIdentical:
      'O Warden não consegue regravar a chave {{key}} sem mudar outra coisa no arquivo, então não oferece o formulário.',
    noKeys: 'O arquivo não tem nenhuma chave para editar.',
  },
  queda: {
    titulo:
      'Esta mudança não pôde ser feita pelo formulário; nada foi alterado. O arquivo está aberto no texto.',
  },
  campo: {
    padrao: 'padrão: {{valor}}',
    deduzido: 'deduzido',
    deduzidoDica: 'Tirado do texto do comentário; pode não ser exato.',
    faixa: 'Faixa: {{min}} a {{max}}.',
    faixaMin: 'Faixa: a partir de {{min}}.',
    faixaMax: 'Faixa: até {{max}}.',
    faixaTexto: 'Faixa: {{texto}}.',
    rotuloDica: 'Nome que o próprio mod dá a esta opção ({{idioma}}).',
    idioma_ptBr: 'pt_br.json do mod',
    idioma_enUs: 'en_us.json do mod, em inglês',
    ligado: 'Ligado',
    desligado: 'Desligado',
    cor: 'Escolher a cor de {{campo}}',
    lista: 'Um item por linha.',
    invalidoInteiro: 'Digite um número inteiro.',
    invalidoNumero: 'Digite um número (use ponto para as casas decimais).',
    foraDaFaixa: '{{valor}} está fora da faixa. O jogo pode trocar por outro valor sem avisar.',
  },
  trecho: {
    abrir: 'Editar este trecho como texto',
    abrirChave: 'Editar este trecho como texto ({{chave}})',
    linha: 'linha {{linha}}',
    listOfObjects: 'Lista de objetos: edite no texto.',
    complexList: 'Lista com itens de tipos diferentes ou listas dentro: edite no texto.',
    nullValue: 'Valor vazio (null): o tipo certo é desconhecido, edite no texto.',
    tooDeep: 'Trecho com muitos níveis: edite no texto.',
  },
  preservar: {
    rotulo: 'Não substituir se o jogador já tiver',
    dica: 'Quem já joga o pack fica com as próprias preferências; o arquivo só chega para quem ainda não tem.',
    foraDoIndice: 'Salve o arquivo uma vez para poder usar esta opção.',
    ligadoToast: '{{arquivo}} não substitui mais o do jogador.',
    desligadoToast: '{{arquivo}} volta a substituir o do jogador.',
    erro: 'Não foi possível mudar a opção de {{arquivo}}.',
  },
  copiar: {
    botao: 'Copiar para defaultconfigs/',
    copiadoToast: 'Copiado para {{destino}}.',
    igualToast: '{{destino}} já tinha este mesmo conteúdo; nada foi gravado.',
    substituirTitulo: 'Substituir {{destino}}?',
    substituirTexto:
      'Já existe um arquivo com outro conteúdo em defaultconfigs/. Ele será trocado por este.',
    substituirConfirmar: 'Substituir',
    salveAntes: 'Salve as alterações antes de copiar.',
    erro: 'Não foi possível copiar {{arquivo}}.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    configsFormulario: typeof configsFormulario;
  }
}
