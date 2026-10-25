/**
 * "Sobre o Warden" (T23): versão, commit e o do packwiz embutido, aviso legal (T01,
 * CA-T01-04), créditos, licenças e o link de apoio ao Forge.
 * O aviso legal é obrigatório e tem esta redação exata (ADR-0010; SPEC T01).
 */
export const sobre = {
  titulo: 'Sobre o Warden',
  versao: 'Versão',
  warden: 'Warden',
  commit: 'Commit',
  packwiz: 'packwiz embutido',
  avisoLegalTitulo: 'Aviso legal',
  creditosTitulo: 'Créditos',
  creditos: {
    packwiz: {
      nome: 'packwiz',
      texto:
        'o formato e a ferramenta de modpacks usados pelo Warden, de comp500 e colaboradores. Vai embutido no app.',
    },
    portablemc: {
      nome: 'portablemc',
      texto: 'o motor que instala e abre o Minecraft nos testes, de Théo Rozier e colaboradores.',
    },
    forge: {
      nome: 'Forge',
      texto:
        'o Warden baixa o Forge sempre do Maven oficial e o instala com o instalador do próprio Forge. O Forge é feito por LexManos e colaboradores, que pedem apoio ao projeto.',
    },
  },
  licencasTitulo: 'Licenças',
  licencasDesc: 'O Warden é feito com componentes de código aberto.',
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
