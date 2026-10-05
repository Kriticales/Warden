import type { JarmetaErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `jarmeta` (QUALITY §3): leitura das informações de um mod direto do
 * arquivo .jar. Falha de disco usa o código comum `IO` (core.ts). Dona: P1-06.
 */
export const jarmeta: Record<JarmetaErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ler as informações de um mod. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_ARCHIVE:
    'O arquivo do mod está corrompido ou não é um .jar de verdade, e não pôde ser lido. Baixe o mod de novo ou remova-o do pack.',
  LIMIT_EXCEEDED:
    'O arquivo do mod é grande demais para ser lido com segurança ({{actual}}, acima do limite de {{limit}}). Confira se é mesmo um mod; se for, baixe-o de novo da página oficial.',
};
