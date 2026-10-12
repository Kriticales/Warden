/**
 * Downloads manuais da CurseForge (SPEC T20; protótipo `teste-manual` em `screens-test.js`;
 * ROADMAP L-06). `etapa` é o texto da etapa que o backend manda enquanto o teste espera
 * (`downloadsManuais:etapa`).
 */
export const downloadsManuais = {
  etapa: 'Esperando os downloads manuais',
  sub: 'Etapa {{atual}} de {{total}} · esperando você',
  titulo_one: '{{count}} mod da CurseForge precisa ser baixado à mão',
  titulo_other: '{{count}} mods da CurseForge precisam ser baixados à mão',
  explicacao:
    'O autor não permite que outros apps baixem estes arquivos. Baixe pelo site e mostre ao Warden onde está.',
  observando: 'O Warden também procura sozinho na pasta Downloads.',
  tabela: {
    rotulo: 'Mods para baixar à mão',
    mod: 'Mod',
    arquivo: 'Arquivo esperado',
    situacao: 'Situação',
    acoes: 'Ações',
  },
  estado: {
    aguardando: 'Aguardando',
    escolhido: 'Encontrado',
    pastaDownloads: 'Encontrado em Downloads',
    errado: 'Este não é o arquivo certo.',
    erradoDetalhe: 'Você escolheu {{escolhido}}. O certo é {{esperado}}.',
  },
  abrirPagina: 'Abrir página de download',
  abrirPaginaDe: 'Abrir página de download de {{mod}}',
  selecionar: 'Selecionar arquivo baixado…',
  selecionarDe: 'Selecionar arquivo baixado de {{mod}}',
  conferindo: 'Conferindo…',
  semPagina: 'Procure {{mod}} no site da CurseForge.',
  continua:
    'Quando todos forem encontrados, o teste continua sozinho. Quem jogar o pack também vai precisar baixar esses mods manualmente.',
  aviso: {
    certo: '{{mod}}: arquivo certo. Ele foi guardado para os próximos testes.',
    errado: '{{mod}}: este não é o arquivo certo. O certo é {{esperado}}.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    downloadsManuais: typeof downloadsManuais;
  }
}
