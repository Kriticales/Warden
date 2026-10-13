/**
 * Navegar por modpacks (SPEC T08, P1-17): Tipo = Modpacks na página de descoberta, a lista
 * **Mods deste modpack** e "Adicionar N selecionados". Protótipo aprovado:
 * `design/prototipo-final/screens-discover.js` (`adicionar-modpacks` e `modpack`). Glossário:
 * QUALITY §8.2.
 */
export const modpacks = {
  verMods: 'Ver os mods',
  verModsDe: 'Ver os mods de {{name}}',
  painel: {
    rotulo: 'Mods do modpack {{name}}',
    resumo: 'por {{author}} · versão {{version}} · {{downloads}} downloads',
    resumoSemAutor: 'versão {{version}} · {{downloads}} downloads',
    alvo: '{{loader}} {{minecraft}}',
    carregando: 'Lendo a lista de mods do modpack…',
    erro: 'Não foi possível abrir este modpack.',
    abrirPagina: 'Abrir a página do modpack',
    fechar: 'Fechar o modpack',
  },
  licenca: {
    nome: 'Licença do modpack: {{license}}.',
    semLicenca: 'A fonte não informa a licença deste modpack.',
    texto:
      'Trazer mods é livre: eles vêm das lojas. As configs do modpack são trabalho do autor e não são copiadas.',
  },
  inteiro:
    'O servidor do arquivo não deixou ler só a lista; o modpack foi baixado inteiro para a memória. Nada foi gravado no computador.',
  mods: {
    titulo: 'Mods deste modpack ({{count}})',
    rotulo: 'Mods do modpack',
    contagem:
      '{{noPack}} já no seu pack · {{compativeis}} compatíveis · {{semVersao}} sem versão para o seu pack · {{fora}} fora das lojas',
    soNovos: 'Só os que não estão no seu pack',
    nenhumNovo: 'Todos os mods deste modpack já estão no seu pack ou não servem para ele.',
    vazio: 'Este modpack não lista nenhum mod.',
    selecionar: 'Selecionar {{name}}',
    semVersao: 'Sem versão para {{alvo}}',
    fora: 'Arquivo fora das lojas: o Warden não copia jars de pacotes de terceiros',
  },
  versao: {
    rotulo: 'Versão dos mods',
    mesma: 'Versão: a mesma do modpack (testada junto)',
    nova: 'Versão: a mais nova compatível',
    outroAlvo:
      'O modpack é de {{alvo}}: a versão de cada mod será a mais nova compatível com o seu pack.',
  },
  selecao: {
    rotulo: 'Mods selecionados',
    contagem_one: '{{count}} selecionado',
    contagem_other: '{{count}} selecionados',
    limpar: 'Limpar seleção',
    adicionar_one: 'Adicionar {{count}} selecionado',
    adicionar_other: 'Adicionar {{count}} selecionados',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    modpacks: typeof modpacks;
  }
}
