/**
 * Perfis do teste (SPEC T11; ROADMAP L-08): consulta e mudanças sobre `test_profile*`.
 *
 * A chave fica dentro da dos Ajustes do teste (`editorKeys.testSettings`): toda mudança de
 * perfil invalida o prefixo inteiro, e assim tudo o que mostra os ajustes do teste fica atual
 * de uma vez (o botão Testar com o perfil ativo, o menu ▾, o diálogo, o "Este teste" da tela
 * do teste e a memória da instância do Prism). O que muda os ajustes por outro caminho
 * (Recriar instância) também invalida o prefixo.
 */
import { useQuery } from '@tanstack/react-query';

import {
  commands,
  type PackId,
  type ProfileFields,
  type TestProfile,
} from '../../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { editorKeys } from '../../pack-editor/api';

export const profileKeys = {
  /** Os perfis do pack (sob a chave dos Ajustes do teste). */
  list: (packId: PackId) => [...editorKeys.testSettings(packId), 'profiles'] as const,
};

export function useTestProfiles(packId: PackId, enabled = true) {
  return useQuery({
    ...commandQuery(profileKeys.list(packId), () => commands.testProfilesGet(packId)),
    staleTime: 0,
    enabled,
  });
}

/** Invalida tudo o que mostra os ajustes do teste do pack. */
function changed(packId: PackId) {
  return { invalidates: [editorKeys.testSettings(packId)] } as const;
}

export function useCreateProfile(packId: PackId) {
  return useCommandMutation(
    ({ name, fields }: { name: string; fields: ProfileFields }) =>
      commands.testProfileCreate(packId, name, fields),
    changed(packId),
  );
}

export function useRenameProfile(packId: PackId) {
  return useCommandMutation(
    ({ id, name }: { id: string; name: string }) => commands.testProfileRename(packId, id, name),
    changed(packId),
  );
}

export function useDeleteProfile(packId: PackId) {
  return useCommandMutation(
    (id: string) => commands.testProfileDelete(packId, id),
    changed(packId),
  );
}

export function useSelectProfile(packId: PackId) {
  return useCommandMutation(
    (id: string) => commands.testProfileSelect(packId, id),
    changed(packId),
  );
}

export function useSaveProfiles(packId: PackId) {
  return useCommandMutation(
    (profiles: TestProfile[]) => commands.testProfilesSave(packId, profiles),
    changed(packId),
  );
}
