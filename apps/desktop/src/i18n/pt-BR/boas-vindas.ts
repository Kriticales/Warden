/**
 * Textos da primeira execução (T01). O aviso legal tem a redação exata da SPEC (ADR-0010) e
 * vem de `sobre.ts`, para os dois lugares nunca divergirem.
 */
export const boasVindas = {
  onde: 'Primeira execução',
  etapas: {
    rotulo: 'Etapas da primeira execução',
    aviso: 'Aviso',
    jogador: 'Nome do jogador',
    pasta: 'Pasta dos packs',
    chaves: 'Chaves (opcional)',
    concluida: ' (concluída)',
    agora: ' (agora)',
  },
  aviso: {
    titulo: 'Boas-vindas ao <hl>Warden</hl>',
    texto:
      'O Warden cria, testa e versiona modpacks no formato packwiz. Para testar, ele abre o Minecraft em modo offline, só neste computador.',
    possuir:
      'Você precisa possuir o Minecraft: Java Edition. O Warden não pede login da Mojang nem da Microsoft.',
    entendi: 'Entendi',
  },
  jogador: {
    titulo: 'Nome do jogador nos testes',
    texto: 'Aparece no jogo e no chat do mundo de teste. Não é a sua conta: o teste é offline.',
  },
  pasta: {
    titulo: 'Pasta dos packs',
    texto: 'Cada pack vira uma pasta aqui dentro. Dá para mudar depois em Configurações.',
    dica: 'Os dados do próprio Warden (instâncias de teste, Java, cache) ficam em outra pasta, fora dos packs.',
  },
  chaves: {
    titulo: 'Chaves',
    texto: 'Todas são opcionais. Sem elas o Warden funciona; só algumas partes ficam de fora.',
    jaConfigurada: 'Já configurada. Deixe em branco para manter.',
    cofreDesc: 'Protegido pelo seu usuário do Windows.',
    envDesc: 'Texto comum na pasta de dados do Warden. Outros programas conseguem ler.',
  },
  voltar: 'Voltar',
  proximo: 'Próximo',
  pular: 'Pular, configuro depois',
  concluir: 'Concluir',
  concluindo: 'Concluindo…',
  naoConcluiu: 'A primeira execução não foi concluída',
  lateral: {
    titulo: 'Onde ficam as coisas',
    packs: 'Seus packs',
    dados: 'Dados do Warden',
    dadosValor: '%APPDATA%\\dev.kriticales.warden',
    chaves: 'Chaves',
    chavesValor: 'Cofre do Windows (ou .env, se você escolher)',
    rodape: 'Nada disso vai para o GitHub sem você publicar uma versão.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    boasVindas: typeof boasVindas;
  }
}
