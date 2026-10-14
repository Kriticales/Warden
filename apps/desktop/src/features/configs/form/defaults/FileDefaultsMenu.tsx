/**
 * O menu ⋯ "Mais ações do arquivo" do editor no modo formulário (protótipo
 * `configs-formulario`): **Restaurar o arquivo todo para o padrão…**, que abre a diferença
 * antes de mudar o editor (C-06). Sem chave diferente do padrão, o item fica desligado.
 */
import { Ellipsis, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Menu, MenuContent, MenuIconTrigger, MenuItem } from '../../../../components/ui/menu';
import type { FormItem } from '../../../../lib/ipc/bindings';
import { changedCount } from './model';

export interface FileDefaultsMenuProps {
  items: readonly FormItem[];
  onRestoreFile: () => void;
}

export function FileDefaultsMenu({ items, onRestoreFile }: FileDefaultsMenuProps) {
  const { t } = useTranslation('configsPadrao');
  const changed = changedCount(items);
  return (
    <span data-defaults-menu="">
      <Menu>
        <MenuIconTrigger icon={Ellipsis} label={t('menu.rotulo')} />
        <MenuContent>
          <MenuItem
            icon={RotateCcw}
            disabled={changed === 0}
            description={changed === 0 ? t('menu.nadaARestaurar') : t('menu.restaurarTudoDesc')}
            onSelect={onRestoreFile}
          >
            {t('menu.restaurarTudo')}
          </MenuItem>
        </MenuContent>
      </Menu>
    </span>
  );
}
