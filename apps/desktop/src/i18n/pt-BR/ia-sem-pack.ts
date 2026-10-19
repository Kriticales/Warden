/**
 * Textos da conversa com a IA sem pack (SPEC T14 item 8; ROADMAP D-19): o botão de Meus packs, a
 * página "Analisar um log", o consentimento e a conversa. O que é igual à conversa com pack (o
 * sintoma, o texto inicial, os selos, a espera, os botões de sempre) vem do namespace `ia`.
 * Glossário (QUALITY §8.2): "conversa", "Enviado à IA", "log", "crash report", "link", "chave".
 * As frases de recusa dos links estão nos erros (`errors/app.ts`, `AI_LINK_*`).
 */
export const iaSemPack = {
  entrada: {
    botao: 'Analisar um log',
    dica: 'Converse com a IA sobre um log ou crash report, sem precisar de um pack.',
  },
  pagina: {
    titulo: 'Analisar um log',
    sub: 'Converse com a IA sobre um log ou crash report, sem precisar de um pack. Você vê tudo o que é enviado.',
    carregando: 'Lendo as configurações da IA…',
    voltar: 'Meus packs',
  },
  nova: {
    titulo: 'Nova análise',
    origem: {
      rotulo: 'Qual log?',
      link: 'Um link colado',
      linkDesc: 'Do mclo.gs ou do pastebin.',
      arquivo: 'Um arquivo do computador',
      arquivoDesc: 'Um log ou crash report.',
    },
    link: {
      rotulo: 'Link do log',
      exemplo: 'https://mclo.gs/…',
      dica: 'O Warden baixa só o texto do link e tira os dados pessoais antes de mostrar. O link não vai para a IA.',
    },
    arquivo: {
      rotulo: 'Arquivo',
      escolher: 'Escolher arquivo…',
      trocar: 'Escolher outro arquivo…',
      aceitos: 'Arquivos .log ou .txt, como o latest.log ou um crash report.',
      dica: '{{tamanho}}. Antes de ir para a IA, o texto passa pela mesma limpeza de dados pessoais dos testes.',
    },
    baixando: 'Baixando o texto do link…',
  },
  consentimento: {
    sub: 'Pedido uma vez para esta conversa. Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada.',
    log: 'O log que você escolheu',
    semPack:
      'Sem pack, a IA não vê lista de mods, testes nem configs, e não faz propostas de mudança.',
  },
  conversas: {
    titulo: 'Conversas sem pack',
    vazio: 'Nenhuma conversa sem pack ainda.',
    onde: 'As conversas e a cópia sem dados pessoais do log ficam nos dados do Warden, neste computador.',
  },
  apagar: {
    texto:
      'A conversa e a cópia do log guardada com ela saem deste computador. O que já foi enviado à IA não volta.',
  },
  erro: {
    guardada: 'A conversa fica guardada.',
    interrompida: 'O Warden fechou antes de a IA responder.',
  },
  perguntar: {
    exemplo: 'Ex.: qual mod aparece primeiro no erro?',
  },
  lado: {
    log: 'o log escolhido',
    github: 'issues no GitHub',
  },
};

declare module '../catalogo' {
  interface Catalogo {
    iaSemPack: typeof iaSemPack;
  }
}
