/**
 * Gaveta de Tarefas (T22) e o indicador do rodapé.
 *
 * `tipos` é o nome de cada tipo de operação (`OperationKind`, como `pack.create`), por chave
 * aninhada (`tipos.pack.create`). Registro acréscimo-apenas: cada tarefa acrescenta os seus
 * tipos aqui. Tipo sem nome cai em `tipoDesconhecido`.
 */
export const tarefas = {
  titulo: 'Tarefas',
  indicador: {
    ociosa: 'Nenhuma tarefa em andamento',
    ocupada_one: '{{count}} tarefa em andamento',
    ocupada_other: '{{count}} tarefas em andamento',
    falha_one: '{{count}} tarefa falhou',
    falha_other: '{{count}} tarefas falharam',
  },
  emAndamento: 'Em andamento',
  concluidas: 'Concluídas',
  carregando: 'Lendo as tarefas…',
  vazio: {
    titulo: 'Nenhuma tarefa ainda',
    texto:
      'Downloads, testes, exportações e outras operações demoradas aparecem aqui enquanto rodam, com o progresso e a opção de cancelar.',
  },
  limite: 'Mostra as últimas 50 tarefas.',
  cancelar: 'Cancelar',
  cancelarNome: 'Cancelar: {{nome}}',
  cancelando: 'Cancelando…',
  aguardandoTrava: 'Aguardando outra operação neste pack',
  verDetalhes: 'Ver detalhes',
  ocultarDetalhes: 'Ocultar detalhes',
  resultado: {
    ok: 'Concluída',
    falhou: 'Falhou',
    cancelada: 'Cancelada',
  },
  linha: '{{nome}} · {{quando}}',
  linhaComResultado: '{{nome}} · {{resultado}} · {{quando}}',
  quando: {
    ontem: 'ontem, {{hora}}',
    data: '{{data}}, {{hora}}',
  },
  progresso: {
    de: '{{feito}} de {{total}}',
    semTotal: '{{feito}}',
  },
  toast: {
    ok: 'Concluída: {{nome}}',
    falhou: 'Falhou: {{nome}}',
    cancelada: 'Cancelada: {{nome}}',
  },
  tipoDesconhecido: 'Tarefa do Warden',
  tipos: {
    test: {
      start: 'Testar o pack',
    },
    java: {
      checkUpdates: 'Procurar atualizações do Java',
    },
    inventory: {
      setSide: 'Alterar lado',
      remove: 'Remover do pack',
      include: 'Incluir arquivos no pack',
    },
    export: {
      run: 'Exportar',
    },
    add: {
      apply: 'Adicionar ao pack',
      // P1-11: link do Modrinth, link direto e arquivo do computador.
      modrinthLink: 'Ler o link do Modrinth',
      directLink: 'Baixar o link',
      localInspect: 'Ler arquivos do computador',
      localApply: 'Adicionar arquivos ao pack',
    },
    versioning: {
      save: 'Salvar versão',
      restore: 'Voltar para uma versão',
      discard: 'Descartar alteração',
    },
    updates: {
      check: 'Verificar atualizações',
      apply: 'Atualizar mods',
    },
    'initial-mods': {
      apply: 'Adicionar os mods iniciais',
    },
    // C-03: O que mudou durante o teste.
    capture: {
      apply: 'Trazer mudanças do teste para o pack',
      discard: 'Descartar mudanças do teste',
      identify: 'Procurar mods colocados à mão',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    tarefas: typeof tarefas;
  }
}
