/**
 * "Sobre o Warden" (T23): versão, aviso legal (T01, CA-T01-04) e o link de apoio ao Forge.
 * O aviso legal é obrigatório e tem esta redação exata (ADR-0010; SPEC T01).
 */
export const sobre = {
  titulo: 'Sobre o Warden',
  versao: 'Versão',
  commit: 'Commit',
  compilacao: 'Compilação',
  compilacaoDesenvolvimento: 'Desenvolvimento (debug)',
  carregando: 'Lendo a versão…',
  avisoLegal:
    'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.',
  offline:
    'O Warden abre o Minecraft em modo offline só para testar seus packs. Você precisa possuir o Minecraft: Java Edition.',
  apoiarForge: 'Apoiar o Forge',
  apoiarForgeDica:
    'O instalador do Forge pede apoio ao projeto. Abre o Patreon do autor no navegador.',
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    sobre: typeof sobre;
  }
}
