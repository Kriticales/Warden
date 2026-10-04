import type { CoreErrorCode } from '../../lib/ipc/bindings';

/** Frases dos erros comuns a todos os domínios (`core`; ARCHITECTURE §5). */
export const core: Record<CoreErrorCode, string> = {
  INTERNAL:
    'Algo deu errado dentro do Warden. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  CANCELLED: 'Operação cancelada.',
  TIMEOUT: 'A operação demorou demais e foi interrompida. Tente de novo.',
  IO: 'Não foi possível ler ou gravar um arquivo em {{path}}. Confira se o disco tem espaço e se outro programa não está usando o arquivo.',
  PATH_OUTSIDE_ROOT:
    'O caminho {{path}} aponta para fora da pasta permitida e foi recusado. Escolha um arquivo dentro da pasta do pack.',
  NETWORK_UNAVAILABLE: 'Sem conexão com a internet. Confira a conexão e tente de novo.',
};
