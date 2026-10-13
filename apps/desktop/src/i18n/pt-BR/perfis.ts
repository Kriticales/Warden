/**
 * Textos dos perfis do teste (SPEC T11 "Perfis do teste"; ROADMAP L-08): o seletor de perfil
 * do diálogo Ajustes do teste, "Ao abrir o jogo", a janela do jogo, os opcionais por perfil, o
 * grupo "Perfil do teste" do menu ▾ e o perfil no "Este teste". Termos do glossário (QUALITY
 * §8.2: "perfil do teste", "instância de teste") e do protótipo aprovado
 * (`design/prototipo-final/screens-test.js` → `settingsDialog`).
 */
export const perfis = {
  padrao: 'Padrão',
  perfil: 'Perfil do teste',
  perfilDica:
    'Cada perfil guarda memória, Java, argumentos, mods opcionais, janela e como o jogo abre. Troque rápido pelo menu ▾ do Testar.',
  emUso: '{{nome}} (em uso no Testar)',
  novo: 'Novo perfil',
  renomear: 'Renomear',
  apagar: 'Apagar',
  padraoFixo: 'O perfil Padrão não pode ser renomeado nem apagado.',
  nome: 'Nome do perfil',
  nomeExemplo: 'Ex.: PC fraco (4 GB)',
  novoDica: 'O perfil novo começa com os ajustes que estão na tela.',
  nomeVazio: 'Dê um nome ao perfil.',
  nomeLongo: 'Use no máximo {{max}} caracteres.',
  nomeRepetido: 'Já existe um perfil com esse nome.',
  criar: 'Criar perfil',
  criando: 'Criando…',
  renomearConfirmar: 'Renomear perfil',
  renomeando: 'Renomeando…',
  cancelar: 'Cancelar',
  criado: 'Perfil "{{nome}}" criado. Para usar, escolha-o no menu ▾ do Testar.',
  renomeado: 'Perfil renomeado.',
  apagarTitulo: 'Apagar o perfil "{{nome}}"?',
  apagarTexto:
    'O perfil sai deste computador. O pack não muda. Se ele estava em uso, o Testar volta para o Padrão.',
  apagarConfirmar: 'Apagar perfil',
  apagando: 'Apagando…',
  apagado: 'Perfil apagado.',
  memoriaAlta:
    'Passa de 75% da memória deste computador ({{total}}). O computador pode ficar lento ou o jogo pode fechar.',
  removerArgumentos: 'Remover estes argumentos',
  efeito: 'Os ajustes valem a partir do próximo teste.',
  abrir: {
    titulo: 'Ao abrir o jogo',
    menu: 'Ir para o menu principal',
    mundo: 'Entrar direto no mundo de teste',
    mundoDica: 'Pula o menu e a escolha do mundo. Usa o Quick Play do próprio jogo.',
    antigo: 'Só no Minecraft 1.20 ou mais novo. Este pack usa o {{minecraft}}.',
    desconhecido:
      'Não dá para saber se a versão do Minecraft deste pack aceita entrar direto no mundo.',
    qualMundo: 'Mundo de teste',
    ultimo: 'O último que você jogou',
    semMundo:
      'Ainda não há mundo de teste nesta instância. Crie um mundo no jogo: nos próximos testes, ele abre direto. Até lá, o jogo abre no menu principal.',
  },
  janela: {
    titulo: 'Janela do jogo',
    padrao: 'Tamanho do jogo (854 × 480)',
    tamanho: '{{largura}} × {{altura}}',
    telaCheia: 'Tela cheia',
  },
  opcionais: {
    titulo: 'Mods opcionais ligados neste perfil',
    texto: 'Sem escolha neste perfil, vale o que está ligado no perfil Padrão.',
  },
  resumo: {
    memoriaAuto: 'Memória automática',
    javaAuto: 'Java automático',
    javaEscolhido: 'Java escolhido',
    mundo: 'entra no mundo',
  },
  esteTeste: 'Perfil',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    perfis: typeof perfis;
  }
}
