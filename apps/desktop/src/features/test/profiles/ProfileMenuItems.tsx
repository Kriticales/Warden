/**
 * Grupo "Perfil do teste" do menu ▾ do Testar (SPEC T11 e T13; ROADMAP L-08): um item de rádio
 * por perfil (`menuitemradio`, como o `menu` do design system), com o resumo à direita. Um
 * clique troca o perfil que o Testar usa; o botão passa a mostrar "Testar · <perfil>".
 */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu';

import { showToast } from '../../../components/ui/toast';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import type { TestMenuContext } from '../menu-items';
import { useSelectProfile, useTestProfiles } from './api';
import { profileName, profileSummary } from './text';

export function ProfileMenuItems({ context }: { context: TestMenuContext }) {
  const profiles = useTestProfiles(context.packId);
  const select = useSelectProfile(context.packId);
  if (!profiles.isSuccess) return null;
  return (
    <MenuPrimitive.RadioGroup
      value={profiles.data.active}
      onValueChange={(id) => {
        if (id === profiles.data.active) return;
        select.mutate(id, {
          onError: (error) => {
            showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
          },
        });
      }}
    >
      {profiles.data.profiles.map((profile) => (
        <MenuPrimitive.RadioItem
          key={profile.id}
          value={profile.id}
          className="menu__item menu__item--radio"
        >
          <span className="menu__radio" aria-hidden="true" />
          <span>{profileName(profile)}</span>
          <span className="t-xs t-3">{profileSummary(profile)}</span>
        </MenuPrimitive.RadioItem>
      ))}
    </MenuPrimitive.RadioGroup>
  );
}
