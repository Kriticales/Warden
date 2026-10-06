/**
 * Menu (shadcn `DropdownMenu` sobre o Radix) com a aparência `.menu` do design system
 * (HANDOFF §3: "Menu"; DESIGN-SYSTEM §6). Abre embaixo do gatilho, ↑ ↓ passam pelos itens,
 * Enter ou Espaço escolhem, Esc fecha e o foco volta para o gatilho (Radix).
 *
 * Cada item tem ícone, texto e, quando ajuda, uma linha de descrição (`description`). Item
 * de perigo (`danger`) leva o ícone e o texto na cor de perigo.
 */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { Icon } from './icon';

export const Menu = MenuPrimitive.Root;
export const MenuTrigger = MenuPrimitive.Trigger;

export interface MenuContentProps {
  /** Nome acessível do menu ("Ações do pack Vale Sereno"). */
  label: string;
  children: ReactNode;
  /** Alinhamento com o gatilho. Padrão: pela ponta direita. */
  align?: 'start' | 'end';
}

export function MenuContent({ label, children, align = 'end' }: MenuContentProps) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content className="menu" aria-label={label} align={align} sideOffset={4}>
        {children}
      </MenuPrimitive.Content>
    </MenuPrimitive.Portal>
  );
}

export interface MenuItemProps {
  icon?: LucideIcon;
  children: ReactNode;
  /** Linha pequena abaixo do texto ("Não apaga nenhum arquivo"). */
  description?: ReactNode;
  /** Ação destrutiva. */
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

export function MenuItem({
  icon,
  children,
  description,
  danger = false,
  disabled = false,
  onSelect,
}: MenuItemProps) {
  return (
    <MenuPrimitive.Item
      className={cn('menu__item', danger && 'menu__item--danger')}
      disabled={disabled}
      onSelect={onSelect}
    >
      {icon ? <Icon icon={icon} /> : <span />}
      <span>{children}</span>
      <span />
      {description ? <span className="menu__desc">{description}</span> : null}
    </MenuPrimitive.Item>
  );
}

export function MenuSeparator() {
  return <MenuPrimitive.Separator className="menu__sep" />;
}
