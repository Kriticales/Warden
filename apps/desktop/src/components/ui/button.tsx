/**
 * Botão do Warden (shadcn `Button` com as classes `.btn` do design system; HANDOFF §3).
 *
 * O degrau de pixel e o chanfro vêm de `components.css` (`::before`/`::after`), por isso não
 * há `rounded-*` aqui. `loading` desabilita, troca o ícone pelo carregador e marca
 * `aria-busy`; o texto no gerúndio é responsabilidade de quem chama (`children`).
 * Botão só de ícone (`iconOnly`) leva o texto como nome acessível e ganha tooltip.
 */
import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';
import type { LucideIcon } from 'lucide-react';
import type { ButtonHTMLAttributes, ReactNode, Ref } from 'react';

import { cn } from '../../lib/cn';
import { Icon } from './icon';
import { Loader } from './loader';
import { Tooltip } from './tooltip';

export const buttonVariants = cva('btn', {
  variants: {
    variant: {
      secondary: 'btn--secondary',
      primary: 'btn--primary',
      ghost: 'btn--ghost',
      danger: 'btn--danger',
      'danger-ghost': 'btn--danger-ghost',
      link: 'btn--link',
    },
    size: {
      md: '',
      sm: 'btn--sm',
      lg: 'btn--lg',
    },
    iconOnly: {
      true: 'btn--icon',
      false: '',
    },
    block: {
      true: 'btn--block',
      false: '',
    },
  },
  defaultVariants: { variant: 'secondary', size: 'md', iconOnly: false, block: false },
});

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
  /** Ícone antes do texto. */
  icon?: LucideIcon;
  /** Ícone depois do texto. */
  iconEnd?: LucideIcon;
  /** Ocupado: desabilita, mostra o carregador e marca `aria-busy`. */
  loading?: boolean;
  /** Contador depois do texto (por exemplo, alterações não salvas). */
  count?: ReactNode;
  /** Renderiza o filho (um `<Link>`, por exemplo) com a aparência do botão. */
  asChild?: boolean;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  variant,
  size,
  iconOnly,
  block,
  icon,
  iconEnd,
  loading = false,
  count,
  asChild = false,
  className,
  children,
  disabled,
  type,
  ref,
  ...props
}: ButtonProps) {
  const classes = cn(buttonVariants({ variant, size, iconOnly, block }), className);
  if (asChild) {
    return (
      <Slot className={classes} ref={ref} {...props}>
        {children}
      </Slot>
    );
  }
  const button = (
    <button
      ref={ref}
      type={type ?? 'button'}
      className={classes}
      disabled={disabled === true || loading}
      aria-busy={loading || undefined}
      {...props}
    >
      {loading ? <Loader /> : null}
      {icon ? <Icon icon={icon} /> : null}
      {iconOnly ? <span className="sr-only">{children}</span> : <span>{children}</span>}
      {count !== undefined ? <span className="btn__count">{count}</span> : null}
      {iconEnd ? <Icon icon={iconEnd} /> : null}
    </button>
  );
  return iconOnly ? <Tooltip content={children}>{button}</Tooltip> : button;
}
