/**
 * Aviso dentro da página (shadcn `Alert` com as classes `.alert`; HANDOFF §3). Ícone e texto,
 * nunca só a cor. Só o de perigo usa `role="alert"` (lido na hora pelo leitor de tela).
 */
import {
  CircleAlert,
  CircleCheck,
  Info,
  Sparkles,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { Icon } from './icon';

export type AlertKind = 'info' | 'ok' | 'warn' | 'danger' | 'neutral' | 'ai';

const ICONS: Record<AlertKind, LucideIcon> = {
  info: Info,
  ok: CircleCheck,
  warn: TriangleAlert,
  danger: CircleAlert,
  neutral: Info,
  ai: Sparkles,
};

export interface AlertProps {
  kind?: AlertKind;
  title?: ReactNode;
  children?: ReactNode;
  /** Botões à direita (ou abaixo, com `actionsBelow`). */
  actions?: ReactNode;
  actionsBelow?: boolean;
  compact?: boolean;
  /** Troca o papel padrão (`alert` só no perigo). */
  role?: 'alert' | 'status' | 'none';
  className?: string | undefined;
}

export function Alert({
  kind = 'info',
  title,
  children,
  actions,
  actionsBelow = false,
  compact = false,
  role,
  className,
}: AlertProps) {
  const effectiveRole = role ?? (kind === 'danger' ? 'alert' : undefined);
  return (
    <div
      className={cn('alert', `alert--${kind}`, compact && 'alert--compact', className)}
      role={effectiveRole === 'none' ? undefined : effectiveRole}
    >
      <Icon icon={ICONS[kind]} />
      <div>
        {title ? <div className="alert__title">{title}</div> : null}
        {children}
      </div>
      {actions ? (
        <div className={cn('alert__actions', actionsBelow && 'alert__actions--below')}>
          {actions}
        </div>
      ) : null}
    </div>
  );
}
