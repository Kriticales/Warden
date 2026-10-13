/**
 * Configurações → Armazenamento (SPEC T21, P1; A-06): o espaço usado pelo Warden e "Limpar
 * cache".
 */
export const armazenamento = {
  titulo: 'Armazenamento',
  descricao:
    'Espaço que o Warden usa neste computador, fora das pastas dos packs. Os tamanhos são medidos ao abrir esta página.',
  legenda: 'Espaço usado pelo Warden',
  itens: {
    cache: 'Cache de downloads',
    minecraft: 'Minecraft e loaders',
    javas: 'Javas',
    instancias: 'Instâncias de teste',
  },
  medindo: 'Medindo…',
  limpar: 'Limpar cache',
  limpando: 'Limpando…',
  emJogo: 'Feche o jogo de {{packName}} para limpar o cache.',
  emTarefa_one: 'Espere {{count}} tarefa em andamento terminar para limpar o cache.',
  emTarefa_other: 'Espere {{count}} tarefas em andamento terminarem para limpar o cache.',
  confirmar: {
    titulo: 'Limpar o cache?',
    texto:
      'Apaga os arquivos baixados guardados e os temporários ({{size}}). Instâncias de teste, mundos, Javas, arquivos do Minecraft e os seus packs ficam. O próximo teste baixa de novo só o que faltar.',
    textoSemTamanho:
      'Apaga os arquivos baixados guardados e os temporários. Instâncias de teste, mundos, Javas, arquivos do Minecraft e os seus packs ficam. O próximo teste baixa de novo só o que faltar.',
    cancelar: 'Manter',
  },
  resultado: {
    limpo: 'Cache limpo: {{size}} liberados.',
    emUso_one: '{{count}} item em uso ficou para a próxima limpeza.',
    emUso_other: '{{count}} itens em uso ficaram para a próxima limpeza.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    armazenamento: typeof armazenamento;
  }
}
