/**
 * Dados do formulário de configs (C-04): o formulário do texto atual (`config_form`), a edição
 * pelo formulário (`config_form_apply`), "Não substituir se o jogador já tiver"
 * (`config_preserve`, `config_set_preserve`) e "Copiar para defaultconfigs/"
 * (`config_copy_to_defaultconfigs`).
 *
 * O formulário é calculado do texto que está no editor (com as alterações não salvas), então a
 * chave da query leva o texto. A edição não grava nada: devolve o texto novo, que o editor
 * grava pelo caminho de sempre. As duas ações que gravam no pack invalidam tudo do pack (a
 * árvore de arquivos, a linha e a lista de Meus packs, a prévia de Exportar e o Histórico).
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';

import { commands, type FormEdit, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { packsKeys } from '../../packs/api';
import { configsKeys } from '../api';

export const formKeys = {
  form: (packId: PackId, path: string, text: string) =>
    [...configsKeys.all(packId), 'form', path, text] as const,
  preserve: (packId: PackId, path: string) =>
    [...configsKeys.all(packId), 'preserve', path] as const,
};

export function useConfigForm(packId: PackId, path: string, text: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(formKeys.form(packId, path, text), () =>
      commands.configForm(packId, path, text),
    ),
    // Enquanto o formulário do texto novo chega, os campos continuam na tela.
    placeholderData: keepPreviousData,
    staleTime: 30_000,
    // A chave leva o texto inteiro (até 2 MB): formulários de textos antigos saem logo do cache.
    gcTime: 10_000,
    enabled,
  });
}

export interface ApplyRequest {
  path: string;
  text: string;
  edits: FormEdit[];
}

/** Aplica as edições ao texto; devolve o texto novo (nada é gravado). */
export function useFormApply() {
  return useCommandMutation(({ path, text, edits }: ApplyRequest) =>
    commands.configFormApply(path, text, edits),
  );
}

export function usePreserve(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(formKeys.preserve(packId, path), () => commands.configPreserve(packId, path)),
    staleTime: 0,
    refetchOnWindowFocus: 'always',
    enabled,
  });
}

/** Tudo o que mostra o pack: a seção inteira, a linha e a lista de Meus packs. */
function packWide(packId: PackId) {
  return [queryKeys.pack(packId), packsKeys.row(packId), packsKeys.list];
}

export function useSetPreserve(packId: PackId) {
  return useCommandMutation(
    ({ path, preserve }: { path: string; preserve: boolean }) =>
      commands.configSetPreserve(packId, path, preserve),
    { invalidates: packWide(packId) },
  );
}

export function useCopyToDefaultconfigs(packId: PackId) {
  return useCommandMutation(
    ({ path, overwrite }: { path: string; overwrite: boolean }) =>
      commands.configCopyToDefaultconfigs(packId, path, overwrite),
    { invalidates: packWide(packId) },
  );
}
