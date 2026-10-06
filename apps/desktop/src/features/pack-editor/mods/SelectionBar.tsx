/**
 * Barra de ações para os itens selecionados (SPEC T06; protótipo `selbar`): aparece quando há
 * seleção. Alterar lado (um único `packwiz refresh` para todos; CA-T06-03) e Remover. Atualizar
 * entra com a P1-12.
 */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu';
import { ChevronDown, Layers, Monitor, Server, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Menu, MenuContent, MenuItem } from '../../../components/ui/menu';
import type { InventoryItem, SideChoice } from '../../../lib/ipc/bindings';
import { SIDE_CHOICES } from './ModRow';

const SIDE_ICONS = { both: Layers, client: Monitor, server: Server } as const;

export interface SelectionBarProps {
  items: readonly InventoryItem[];
  busy: boolean;
  onSide: (side: SideChoice) => void;
  onRemove: () => void;
  onClear: () => void;
}

export function SelectionBar({ items, busy, onSide, onRemove, onClear }: SelectionBarProps) {
  const { t } = useTranslation('editor');
  const editable = items.filter((item) => item.sideEditable).length;
  return (
    <div className="selbar mods-selbar" role="region" aria-label={t('mods.selecao.rotulo')}>
      <span className="selbar__count" aria-live="polite">
        {t('mods.selecao.contagem', { count: items.length })}
      </span>
      <Menu>
        <MenuPrimitive.Trigger
          className={buttonVariants({ size: 'sm' })}
          disabled={busy || editable === 0}
          title={editable === 0 ? t('mods.selecao.ladoNaoEditavel') : undefined}
        >
          <span>{t('mods.selecao.alterarLado')}</span>
          <Icon icon={ChevronDown} />
        </MenuPrimitive.Trigger>
        <MenuContent align="start">
          {SIDE_CHOICES.map((side) => (
            <MenuItem
              key={side}
              icon={SIDE_ICONS[side]}
              onSelect={() => {
                onSide(side);
              }}
            >
              {t('mods.selecao.ladoPara', { lado: t(`mods.lado.${side}`) })}
            </MenuItem>
          ))}
        </MenuContent>
      </Menu>
      <Button size="sm" variant="danger-ghost" icon={Trash2} onClick={onRemove}>
        {t('mods.selecao.remover')}
      </Button>
      <span className="grow" />
      <Button size="sm" variant="ghost" onClick={onClear}>
        {t('mods.selecao.limpar')}
      </Button>
    </div>
  );
}
