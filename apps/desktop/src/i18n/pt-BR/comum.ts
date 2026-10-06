/**
 * Textos comuns do app (QUALITY §8). Todo texto visível vem de um catálogo como este.
 */
export const comum = {
  app: {
    descricao: 'Crie, teste e diagnostique modpacks no formato packwiz.',
    boasVindas: 'Boas-vindas ao <hl>Warden</hl>',
  },
  acoes: {
    fechar: 'Fechar',
    fecharAviso: 'Fechar aviso',
    fecharPainel: 'Fechar painel',
    cancelar: 'Cancelar',
    tentarDeNovo: 'Tentar de novo',
    abrirNoNavegador: 'Abrir no navegador',
  },
  carregando: 'Carregando…',
  // O Radix troca {hotkey} pelo atalho (F8); chaves simples não são interpolação do i18next.
  avisos: 'Avisos ({hotkey})',
  erro: {
    detalhesTecnicos: 'Detalhes técnicos',
    codigo: 'Código',
    detalhe: 'Detalhe',
    operacao: 'Tarefa',
    copiar: 'Copiar',
    copiado: 'Detalhes copiados.',
    naoCopiou: 'Não foi possível copiar. Selecione o texto e copie com Ctrl+C.',
    parametroAusente: 'valor desconhecido',
    telaTitulo: 'Esta tela não abriu',
  },
  confirmar: {
    digite: 'Para confirmar, digite <code>{{texto}}</code>',
    campo: 'Texto de confirmação',
    naoConfere: 'O texto digitado não confere.',
  },
  diff: {
    adicionada: 'Adicionada: ',
    removida: 'Removida: ',
    resumo: '{{adicionadas}} linhas adicionadas e {{removidas}} removidas',
    oculto_one: '{{count}} linha igual oculta',
    oculto_other: '{{count}} linhas iguais ocultas',
  },
  conteudo: {
    video: 'Vídeo incorporado',
    videoDe: 'Vídeo de {{host}}',
    miniatura: 'Miniatura do vídeo',
    linkBloqueado: 'Link sem endereço seguro (só https é aberto)',
  },
  progresso: {
    concluido: '{{valor}}%',
  },
  etapas: {
    concluida: ' (concluída)',
    agora: ' (agora)',
  },
} as const;
