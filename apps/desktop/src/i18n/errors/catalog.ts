import type { CatalogErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `catalog` (QUALITY §3): listas de versões do Minecraft e dos
 * loaders. `{{source}}` é a fonte que falhou (Mojang, Fabric, Forge ou NeoForge). Sem internet e
 * sem lista guardada, o erro é o comum `core.NETWORK_UNAVAILABLE`. Dona: P1-05.
 */
export const catalog: Record<CatalogErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao buscar as versões do Minecraft e dos loaders. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  SOURCE_UNAVAILABLE:
    'O servidor de versões do {{source}} está fora do ar ou com problemas, e o Warden ainda não tem a lista guardada. Tente de novo em alguns minutos.',
  RATE_LIMITED:
    'O servidor de versões do {{source}} pediu para esperar antes de receber mais pedidos. Aguarde um minuto e tente de novo.',
  REQUEST_REJECTED:
    'O servidor de versões do {{source}} recusou o pedido (erro {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_RESPONSE:
    'O servidor de versões do {{source}} respondeu algo que o Warden não entende. Tente de novo mais tarde; se continuar, copie os detalhes técnicos e registre o problema.',
  VERSION_NOT_FOUND:
    'A versão {{id}} do Minecraft não existe na lista oficial da Mojang. Escolha outra versão na lista.',
  VERSION_JSON_CORRUPTED:
    'Os dados da versão {{id}} do Minecraft chegaram diferentes do esperado e foram descartados. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  CACHE_UNAVAILABLE:
    'O Warden não conseguiu usar a lista de versões guardada no disco. Confira se há espaço livre e tente de novo.',
};
