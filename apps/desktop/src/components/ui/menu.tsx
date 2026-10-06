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
import { buttonVariants } from './button';
import { Icon } from './icon';
import { Tooltip } from './tooltip';

/**
 * Raiz do menu, não modal: itens que abrem um diálogo (Remover, Apagar…) não deixam a página
 * presa com `pointer-events: none` quando o diálogo fecha (o menu modal e o diálogo disputam o
 * mesmo estilo do `<body>`).
 */
export function Menu({ children }: { children: ReactNode }) {
  return <MenuPrimitive.Root modal={false}>{children}</MenuPrimitive.Root>;
}

export const MenuTrigger = MenuPrimitive.Trigger;

export interface MenuIconTriggerProps {
  icon: LucideIcon;
  /** Nome acessível e texto da dica ("Mais ações para Vale Sereno"). */
  label: string;
}

/**
 * Gatilho só de ícone (o ⋯ das linhas), com dica. O `Button` com `iconOnly` já vem embrulhado
 * no `Tooltip`, que não repassa as props do `MenuTrigger`; aqui a ordem é dica → gatilho → botão.
 */
export function MenuIconTrigger({ icon, label }: MenuIconTriggerProps) {
  return (
    <Tooltip content={label}>
      <MenuPrimitive.Trigger
        className={buttonVariants({ variant: 'ghost', size: 'sm', iconOnly: true })}
      >
        <Icon icon={icon} />
        <span className="sr-only">{label}</span>
      </MenuPrimitive.Trigger>
    </Tooltip>
  );
}

export interface MenuContentProps {
  children: ReactNode;
  /** Alinhamento com o gatilho. Padrão: pela ponta direita. */
  align?: 'start' | 'end';
}

/** O conteúdo do menu. O nome acessível é o do gatilho (`aria-labelledby`, pelo Radix). */
export function MenuContent({ children, align = 'end' }: MenuContentProps) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content className="menu" align={align} sideOffset={4}>
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
