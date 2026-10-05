/**
 * Toasts (Radix Toast, via shadcn) com a aparência `.toast` (HANDOFF §3 e §4).
 *
 * Confirmam algo que acabou de acontecer: sobem 8 px em 200 ms, somem em 6 s, pausam no hover
 * e no foco (F8 leva o foco para a fila). Erro é anunciado na hora (`foreground`). Um toast
 * nunca é a única cópia de um erro que exige ação: o erro também fica na tela ou em Tarefas.
 *
 * Qualquer parte do app chama `showToast(...)`; o `<Toaster />` fica na raiz.
 */
import * as ToastPrimitive from '@radix-ui/react-toast';
import { CircleAlert, CircleCheck, Info, TriangleAlert, X, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { create } from 'zustand';

import { Button } from './button';
import { Icon } from './icon';

/** Tempo na tela (HANDOFF §4). */
export const TOAST_DURATION_MS = 6000;

export type ToastKind = 'info' | 'ok' | 'warn' | 'danger';

export interface ToastAction {
  /** Texto do botão (verbo + objeto). */
  label: string;
  /** Texto para quem não vê o toast (Radix `altText`): onde a mesma ação existe. */
  altText: string;
  onClick: () => void;
}

export interface ToastInput {
  kind?: ToastKind;
  title: ReactNode;
  text?: ReactNode;
  action?: ToastAction;
}

export interface ToastItem extends ToastInput {
  id: number;
  kind: ToastKind;
}

interface ToastStore {
  toasts: ToastItem[];
  show: (toast: ToastInput) => number;
  dismiss: (id: number) => void;
  clear: () => void;
}

let nextId = 1;

export const useToastStore = create<ToastStore>()((set) => ({
  toasts: [],
  show: (toast) => {
    const id = nextId;
    nextId += 1;
    set((state) => ({ toasts: [...state.toasts, { ...toast, kind: toast.kind ?? 'info', id }] }));
    return id;
  },
  dismiss: (id) => {
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) }));
  },
  clear: () => {
    set({ toasts: [] });
  },
}));

/** Mostra um toast; devolve o id (para `dismissToast`). */
export function showToast(toast: ToastInput): number {
  return useToastStore.getState().show(toast);
}

export function dismissToast(id: number): void {
  useToastStore.getState().dismiss(id);
}

const ICONS: Record<ToastKind, LucideIcon> = {
  info: Info,
  ok: CircleCheck,
  warn: TriangleAlert,
  danger: CircleAlert,
};

function ToastView({ toast }: { toast: ToastItem }) {
  const { t } = useTranslation();
  const dismiss = useToastStore((state) => state.dismiss);
  return (
    <ToastPrimitive.Root
      className={`toast toast--${toast.kind}`}
      type={toast.kind === 'danger' ? 'foreground' : 'background'}
      duration={TOAST_DURATION_MS}
      onOpenChange={(open) => {
        if (!open) dismiss(toast.id);
      }}
    >
      <Icon icon={ICONS[toast.kind]} />
      <div className="toast__text">
        <ToastPrimitive.Title asChild>
          <strong>{toast.title}</strong>
        </ToastPrimitive.Title>
        {toast.text ? <ToastPrimitive.Description>{toast.text}</ToastPrimitive.Description> : null}
      </div>
      <div className="toast__actions">
        {toast.action ? (
          <ToastPrimitive.Action altText={toast.action.altText} asChild>
            <Button variant="ghost" size="sm" onClick={toast.action.onClick}>
              {toast.action.label}
            </Button>
          </ToastPrimitive.Action>
        ) : null}
        <ToastPrimitive.Close asChild>
          <Button variant="ghost" size="sm" iconOnly icon={X}>
            {t('acoes.fecharAviso')}
          </Button>
        </ToastPrimitive.Close>
      </div>
    </ToastPrimitive.Root>
  );
}

/** Fila de toasts. Uma vez, na raiz do app. */
export function Toaster() {
  const { t } = useTranslation();
  const toasts = useToastStore((state) => state.toasts);
  return (
    <ToastPrimitive.Provider swipeDirection="right" duration={TOAST_DURATION_MS}>
      {toasts.map((toast) => (
        <ToastView key={toast.id} toast={toast} />
      ))}
      <ToastPrimitive.Viewport className="toasts" label={t('avisos')} />
    </ToastPrimitive.Provider>
  );
}
