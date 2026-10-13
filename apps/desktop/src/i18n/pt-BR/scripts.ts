/**
 * Editor de scripts KubeJS e CraftTweaker (SPEC T26; protótipo `scripts`; C-07): a barra do
 * editor com a versão, os trechos prontos, "Recarregar no jogo" e "Abrir no VS Code", o painel
 * "Erros dos scripts", o autocompletar de IDs e o "Recarregar scripts" da tela do teste.
 */
export const scripts = {
  barra: {
    versao: 'Ferramenta de scripts do pack',
    trechos: 'Trechos prontos',
    trechosDica: 'Modelos dos eventos mais usados desta versão',
    recarregar: 'Recarregar no jogo',
    recarregando: 'Recarregando…',
    salvarERecarregar: 'Salvar e recarregar',
    vscode: 'Abrir no VS Code',
    vscodeSemInstancia: 'Teste o pack uma vez para criar a instância de teste.',
    semJogo: 'Abra o jogo pelo Testar para recarregar os scripts sem reiniciar.',
  },
  trechos: {
    'receita-com-forma': 'Receita com forma',
    'receita-sem-forma': 'Receita sem forma',
    'remover-receita': 'Remover receita',
    'tag-de-item': 'Tag de item',
    'item-novo': 'Item novo',
    'dica-de-item': 'Dica de item',
    fornalha: 'Receita de fornalha',
  },
  comando: {
    titulo: 'No {{ferramenta}}, recarregar precisa de um comando no jogo.',
    texto:
      'Cole no chat do jogo e aperte Enter. Com o servidor local aberto, o Warden envia sozinho.',
    copiar: 'Copiar {{comando}}',
    copiado: 'Comando copiado',
    fechar: 'Fechar o aviso',
  },
  reiniciar: {
    titulo: 'O CraftTweaker do Minecraft 1.12.2 não recarrega scripts com o jogo aberto.',
    texto: 'Feche o jogo e teste de novo para ver a mudança.',
  },
  recarregado: {
    jogo: 'Scripts recarregados no jogo',
    servidor: 'Scripts recarregados no servidor',
    servidorTexto: 'O Warden enviou {{comando}}.',
  },
  jogoAberto: {
    titulo: 'O jogo está aberto com a instância de teste.',
    texto:
      'O que você salvar aqui vale no próximo teste. Para recarregar agora, edite o script na instância de teste.',
    acao: 'Editar na instância de teste',
  },
  erros: {
    titulo: 'Erros dos scripts',
    local: '{{arquivo}}, linha {{linha}}',
    localColuna: '{{arquivo}}, linha {{linha}}, coluna {{coluna}}',
    aviso: 'Aviso',
    vazio: 'Nenhum erro no último teste.',
    semLogs: 'Ainda não há logs dos scripts. Teste o pack para ver os erros aqui.',
    meta: 'do último teste, {{quando}} · lido de {{arquivos}}',
    aoVivo: 'ao vivo, do jogo aberto',
    hoje: 'hoje às {{hora}}',
    ontem: 'ontem às {{hora}}',
    outroDia: 'em {{data}} às {{hora}}',
    carregando: 'Lendo os logs…',
    erro: 'Não foi possível ler os erros dos scripts.',
  },
  ids: {
    item: 'item',
    block: 'bloco',
    itemTag: 'tag de item',
    blockTag: 'tag de bloco',
    fluidTag: 'tag de fluido',
    recipe: 'receita',
    jogoAberto: 'do jogo aberto',
  },
  teste: {
    recarregar: 'Recarregar scripts',
    dica: 'Recarrega os scripts do {{ferramenta}} sem fechar o jogo',
    altCopiar: 'O mesmo comando aparece ao recarregar pelo editor de scripts, em Configs.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    scripts: typeof scripts;
  }
}
