import type { DiscoveryErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `discovery` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const discovery: Record<DiscoveryErrorCode, string> = {
  INTERNAL:
    'Algo deu errado na página de descoberta. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  MODPACK_BLOCKED:
    'Este modpack não pode ser aberto por apps de terceiros. Abra a página na CurseForge.',
  MODPACK_TOO_LARGE:
    'O servidor não deixa ler só uma parte deste modpack, e o arquivo inteiro passa de 200 MB. O Warden não baixa arquivos desse tamanho só para listar os mods.',
  MODPACK_UNREADABLE:
    'Não foi possível ler a lista de mods deste modpack. O arquivo do modpack pode estar com defeito.',
};
