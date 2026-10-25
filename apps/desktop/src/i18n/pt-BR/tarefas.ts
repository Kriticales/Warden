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
    // B1-C: os tipos que ainda caíam em "Tarefa do Warden".
    pack: {
      create: 'Criar pack',
    },
    diagnostics: {
      check: 'Verificar o pack',
    },
    configs: {
      write: 'Salvar config',
      preserve: 'Mudar "Não substituir se o jogador já tiver"',
      copyDefaultconfigs: 'Copiar config para defaultconfigs/',
      bring: 'Trazer config para o pack',
      personal: 'Mudar configs pessoais',
    },
    scripts: {
      copyToInstance: 'Copiar script para a instância de teste',
    },
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
      setOptional: 'Mudar "Opcional para o jogador"',
      setPinned: 'Fixar ou soltar a versão',
    },
    export: {
      run: 'Exportar',
      formatRun: 'Exportar .mrpack ou .zip da CurseForge',
      prism: 'Gerar a instância para o Prism',
      serverPack: 'Gerar o pacote para servidor',
    },
    add: {
      apply: 'Adicionar ao pack',
      // P1-11: link do Modrinth, link direto e arquivo do computador.
      modrinthLink: 'Ler o link do Modrinth',
      curseforgeLink: 'Ler o link da CurseForge',
      directLink: 'Baixar o link',
      localInspect: 'Ler arquivos do computador',
      localApply: 'Adicionar arquivos ao pack',
      // P1-17: a lista de mods de um modpack (cancelada ao fechar o painel).
      modpackContents: 'Ler os mods do modpack',
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
    versions: {
      change: 'Trocar de versão',
      loader: 'Trocar a versão do loader',
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
    publish: {
      run: 'Publicar versão',
    },
    // P1-19: Importar modpack.
    import: {
      analyze: 'Ler o modpack',
      apply: 'Importar modpack',
    },
    // D-12: busca do culpado.
    bisect: {
      run: 'Encontrar o mod culpado',
    },
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    tarefas: typeof tarefas;
  }
}
