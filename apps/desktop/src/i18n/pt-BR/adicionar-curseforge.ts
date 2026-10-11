/**
 * CurseForge na página Adicionar (SPEC T08; P1-10): "Download manual necessário", conferência
 * do arquivo entre as fontes e o link da CurseForge colado no campo único. Os avisos de chave
 * ausente ou recusada e o resto da página estão em `adicionar.ts`. Glossário: QUALITY §8.2.
 */
export const adicionarCurseforge = {
  downloadManual: 'Download manual necessário',
  downloadManualDica:
    'O autor bloqueou downloads por outros apps. Quem joga vai precisar baixar este arquivo à mão.',
  downloadManualAviso: {
    titulo: 'Download manual necessário',
    texto:
      'O autor de {{name}} não deixa outros apps baixarem este mod da CurseForge. Ele entra no pack, mas quem instalar o pack vai precisar baixar o arquivo à mão pelo site. Se o mesmo mod existir no Modrinth, prefira o Modrinth.',
  },
  conferencia: {
    lendo: 'Conferindo o arquivo nas duas fontes…',
    igual: 'O arquivo desta versão é o mesmo no Modrinth e na CurseForge (o SHA-1 confere).',
    diferente: 'O arquivo desta versão não é o mesmo nas duas fontes.',
    diferenteTexto:
      'Nenhuma versão no {{fonte}} tem o mesmo SHA-1. Escolha a fonte que você prefere e confira a versão antes de adicionar.',
  },
  link: {
    lendo: 'Lendo o link da CurseForge…',
    lendoArquivo: 'O packwiz está lendo o arquivo numa cópia do pack. Nada foi gravado no pack.',
    erro: 'Não foi possível ler este link da CurseForge.',
    projetoLido: 'Link de {{name}} lido.',
    arquivoLido: 'Link do arquivo de {{name}} lido.',
    verProjeto: 'Ver {{name}}',
    adicionarArquivo: 'Adicionar {{name}}',
    abrirConfiguracoes: 'Abrir Configurações',
    semChave: 'Para ler links da CurseForge, informe sua chave em Configurações.',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    adicionarCurseforge: typeof adicionarCurseforge;
  }
}
