import type { InstanceErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `instance` (QUALITY §3): a etapa "Copiar o pack para o teste" e o
 * cache de downloads. Criado pela F0-05 só com `INTERNAL`; a L-03 acrescentou os códigos da
 * materialização.
 */
export const instance: Record<InstanceErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao preparar a instância de teste. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  PACK_UNREADABLE:
    'O Warden não conseguiu ler o pack.toml ou o index.toml deste pack. Abra a seção Problemas para ver o que está errado.',
  MINECRAFT_VERSION_MISSING:
    'O pack não diz a versão do Minecraft. Escolha a versão em Informações do pack e tente de novo.',
  LOADER_VERSION_NOT_EXACT:
    'A versão do {{loader}} no pack ("{{version}}") não é uma versão exata. Escolha a versão do loader em Informações do pack e tente de novo.',
  LOADER_AMBIGUOUS:
    'O pack tem mais de um loader ({{loaders}}). Deixe só um em Informações do pack e tente de novo.',
  SYNC_INCOMPLETE:
    'Não foi possível copiar {{count}} itens do pack para o teste: {{names}}. Confira a internet e tente de novo; a lista completa está nos detalhes técnicos.',
  MANUAL_FILE_MISMATCH:
    'Este não é o arquivo certo. Baixe {{file}} pela página indicada e escolha esse arquivo.',
  CACHE_UNAVAILABLE:
    'O Warden não conseguiu usar a pasta de downloads guardados. Feche outros programas que possam estar usando essa pasta e tente de novo.',
};
