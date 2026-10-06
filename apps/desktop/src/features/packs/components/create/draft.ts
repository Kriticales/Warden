/**
 * O rascunho do assistente Criar pack e a leitura dos erros da etapa "Nome e pasta".
 */
import type { AppError, Loader } from '../../../../lib/ipc/bindings';

export interface Draft {
  name: string;
  /** Em branco: o Rust usa o nome do jogador configurado. */
  author: string;
  description: string;
  /** `null` = `<pasta dos packs>\<nome-em-minúsculas-com-hífens>`. */
  destination: string | null;
  minecraft: string | null;
  /** `null` = Nenhum (vanilla). */
  loader: Loader | null;
  loaderVersion: string | null;
  /** O loader já foi escolhido (ou pré-selecionado) para a versão do Minecraft atual. */
  loaderReady: boolean;
}

/** O que há de errado no nome ou na pasta, com a chave do texto em `packs.criar`. */
export type CheckProblem =
  | { field: 'name'; key: 'nome.vazio' | 'nome.longo' | 'nome.invalido' }
  | { field: 'path'; key: 'pasta.cheia' | 'pasta.relativa' };

/**
 * Lê o erro de `pack_create_check` (ou de `pack_create`) como um problema de campo. Outros
 * erros (disco, permissão) voltam `null` e aparecem no painel de erro comum.
 */
export function checkProblem(error: AppError | null, name = ''): CheckProblem | null {
  if (error?.code.domain !== 'project') return null;
  if (error.code.code === 'DESTINATION_NOT_EMPTY') {
    return { field: 'path', key: 'pasta.cheia' };
  }
  if (error.code.code !== 'INVALID_INPUT') return null;
  if (error.params.field === 'path') return { field: 'path', key: 'pasta.relativa' };
  if (error.params.field !== 'name') return null;
  const trimmed = name.trim();
  if (trimmed === '') return { field: 'name', key: 'nome.vazio' };
  // Conta caracteres como o Rust (`chars().count()`), não unidades UTF-16.
  if (Array.from(trimmed).length > 64) return { field: 'name', key: 'nome.longo' };
  return { field: 'name', key: 'nome.invalido' };
}
