import type { ImportErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `import` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a P1-19
 * acrescentou uma frase para cada código novo.
 */
export const importErrors: Record<ImportErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao importar o pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  UNKNOWN_FORMAT:
    '{{name}} não é um modpack que o Warden importa. Escolha um .mrpack, um zip da CurseForge ou a pasta (ou o zip) de uma instância do Prism, do MultiMC ou do app da CurseForge.',
  UNREADABLE:
    'Não foi possível ler {{name}}: o arquivo parece corrompido ou incompleto. Baixe de novo e tente outra vez. Nada foi criado.',
  UNSAFE_PATH:
    'Este modpack tem um caminho perigoso ({{path}}), que poderia gravar fora da pasta do pack. Por segurança, o Warden recusou o arquivo inteiro. Nada foi criado.',
  UNSUPPORTED_LOADER:
    'Este modpack usa um loader que o Warden ainda não suporta, ou mais de um loader. Packs Forge, NeoForge e Fabric são importados normalmente. Nada foi criado.',
  CURSEFORGE_KEY_MISSING:
    'Para importar um modpack da CurseForge, o Warden precisa da sua chave da CurseForge. Cadastre-a em Configurações, em Chaves e contas. Nada foi gravado.',
  CURSEFORGE_IMPORT_FAILED:
    'O packwiz não conseguiu ler este zip da CurseForge. Os detalhes técnicos dizem o que ele respondeu. Nada foi criado.',
  TOO_LARGE:
    '{{name}} é grande demais ou tem arquivos demais para importar com segurança. Nada foi criado.',
  SOURCE_CHANGED:
    '{{name}} mudou depois que o Warden leu. Leia o modpack de novo antes de importar. Nada foi criado.',
  PLAN_EXPIRED:
    'A revisão desta importação expirou. Escolha o modpack de novo para o Warden ler outra vez. Nada foi criado.',
  MINECRAFT_MISSING:
    'O modpack não diz para qual versão do Minecraft ele é. O Warden não consegue criar o pack sem essa informação. Nada foi criado.',
};
