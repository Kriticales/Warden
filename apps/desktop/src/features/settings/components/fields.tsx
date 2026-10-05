/**
 * Controles de formulário de Configurações e da primeira execução, com a marcação das classes
 * do design system (`.field`, `.input`, `.select`, `.switch`, `.check`, `.choice`, `.status`;
 * `design/system/components.js`). Rótulo, dica e erro ficam ligados ao controle por
 * `aria-describedby`, e o erro marca `aria-invalid`.
 */
import { CircleAlert, CircleCheck, CircleHelp, CircleX, TriangleAlert } from 'lucide-react';
import { useId, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from 'react';

import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';

/** Ids do erro e da dica de um campo, para `aria-describedby`. */
export function describedBy(id: string, hint: ReactNode, error: ReactNode): string | undefined {
  const ids = [error ? `${id}-err` : '', hint ? `${id}-hint` : ''].filter(Boolean);
  return ids.length > 0 ? ids.join(' ') : undefined;
}

interface FieldShellProps {
  id: string;
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  className?: string;
  children: ReactNode;
}

/** Rótulo, controle, erro e dica, na ordem do design system. */
export function FieldShell({ id, label, hint, error, className, children }: FieldShellProps) {
  return (
    <div className={cn('field', className)}>
      {label ? (
        <label className="field__label" htmlFor={id}>
          {label}
        </label>
      ) : null}
      {children}
      {error ? (
        <div className="field__error" id={`${id}-err`} role="alert">
          <Icon icon={CircleAlert} size="sm" />
          <span>{error}</span>
        </div>
      ) : null}
      {hint ? (
        <div className="field__hint" id={`${id}-hint`}>
          {hint}
        </div>
      ) : null}
    </div>
  );
}

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'id'> {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  mono?: boolean;
  /** Só o `<input>`, sem rótulo nem dica (o rótulo vem por `aria-label`). */
  bare?: boolean;
  id?: string | undefined;
}

export function TextField({
  label,
  hint,
  error,
  mono = false,
  bare = false,
  id: givenId,
  className,
  ...props
}: TextFieldProps) {
  const autoId = useId();
  const id = givenId ?? autoId;
  const input = (
    <input
      id={id}
      type="text"
      autoComplete="off"
      spellCheck={false}
      className={cn('input', mono && 'input--mono', className)}
      aria-invalid={error ? true : undefined}
      aria-describedby={bare ? undefined : describedBy(id, hint, error)}
      {...props}
    />
  );
  if (bare) {
    return input;
  }
  return (
    <FieldShell id={id} label={label} hint={hint} error={error}>
      {input}
    </FieldShell>
  );
}

export interface SelectFieldProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'id'> {
  label: ReactNode;
  hint?: ReactNode;
  options: readonly { value: string; label: string }[];
}

export function SelectField({ label, hint, options, className, ...props }: SelectFieldProps) {
  const id = useId();
  return (
    <FieldShell id={id} label={label} hint={hint}>
      <select
        id={id}
        className={cn('select', className)}
        aria-describedby={describedBy(id, hint, null)}
        {...props}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </FieldShell>
  );
}

export interface SwitchProps {
  label: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  onText: string;
  offText: string;
}

/** Interruptor (`role="switch"`) com o estado escrito ao lado. */
export function Switch({
  label,
  checked,
  onChange,
  disabled = false,
  onText,
  offText,
}: SwitchProps) {
  const id = useId();
  return (
    <span className={cn('switch', disabled && 'switch--disabled')}>
      <button
        type="button"
        role="switch"
        className="switch__track"
        id={id}
        aria-checked={checked}
        aria-labelledby={`${id}-l`}
        disabled={disabled}
        onClick={() => {
          onChange(!checked);
        }}
      />
      <label id={`${id}-l`} htmlFor={id}>
        {label}
      </label>
      <span className="switch__state" aria-hidden="true">
        {checked ? onText : offText}
      </span>
    </span>
  );
}

export interface CheckboxProps {
  label: ReactNode;
  desc?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function Checkbox({ label, desc, checked, onChange, disabled = false }: CheckboxProps) {
  return (
    <label className="check">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      <span className="check__text">
        <span>{label}</span>
        {desc ? <span className="check__desc">{desc}</span> : null}
      </span>
    </label>
  );
}

export interface ChoiceProps {
  name: string;
  title: ReactNode;
  desc: ReactNode;
  badge?: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

/** Opção grande de rádio (`.choice`), para escolhas com explicação. */
export function Choice({
  name,
  title,
  desc,
  badge,
  checked,
  disabled = false,
  onSelect,
}: ChoiceProps) {
  const id = useId();
  return (
    <label className="choice" htmlFor={id}>
      <span className="check">
        <input
          type="radio"
          id={id}
          name={name}
          checked={checked}
          disabled={disabled}
          aria-describedby={`${id}-desc`}
          onChange={() => {
            onSelect();
          }}
        />
      </span>
      <span className="grow">
        <span className="choice__title">{title}</span>
        {badge}
        <span className="choice__desc block" id={`${id}-desc`}>
          {desc}
        </span>
      </span>
    </label>
  );
}

const STATUS_ICONS = {
  ok: CircleCheck,
  danger: CircleX,
  warn: TriangleAlert,
  muted: CircleHelp,
} as const;

/** Estado curto com ícone ("Configurada", "Funcionando"). */
export function StatusText({
  kind,
  children,
}: {
  kind: keyof typeof STATUS_ICONS;
  children: ReactNode;
}) {
  return (
    <span className={`status status--${kind}`}>
      <Icon icon={STATUS_ICONS[kind]} size="sm" />
      {children}
    </span>
  );
}
