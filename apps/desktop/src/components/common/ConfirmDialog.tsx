/**
 * Confirmação (HANDOFF §3; DESIGN-SYSTEM §6): `AlertDialog` com o título que repete a ação, o
 * que vai acontecer e o botão com verbo + objeto ("Remover JEI"), nunca "OK" ou "Sim".
 *
 * Com `confirmText`, o botão só habilita depois de digitar exatamente aquele texto (por
 * exemplo, o nome do pack antes de apagá-lo). `onConfirm` pode ser assíncrono: o botão fica
 * ocupado, o diálogo fecha quando der certo e mostra o `ErrorPanel` quando falhar.
 */
import { useId, useState, type ReactNode } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogTrigger,
} from '../ui/alert-dialog';
import { Button } from '../ui/button';
import { ErrorPanel } from './ErrorPanel';

export interface ConfirmDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** Elemento que abre o diálogo (quando não é controlado por `open`). */
  trigger?: ReactNode;
  title: ReactNode;
  /** O que vai acontecer (e o que não vai). */
  description: ReactNode;
  /** Texto do botão de confirmação: verbo + objeto. */
  confirmLabel: ReactNode;
  /** Texto do botão enquanto a ação roda, no gerúndio ("Removendo…"). */
  confirmingLabel?: ReactNode;
  /** Texto do botão que desiste. Padrão: "Cancelar". */
  cancelLabel?: ReactNode;
  /** Ação destrutiva (botão de perigo). Padrão: sim. */
  destructive?: boolean;
  /** Texto que precisa ser digitado para habilitar a confirmação. */
  confirmText?: string;
  onConfirm: () => void | Promise<void>;
  /** Conteúdo extra entre a descrição e o rodapé. */
  children?: ReactNode;
}

export function ConfirmDialog({
  open,
  onOpenChange,
  trigger,
  title,
  description,
  confirmLabel,
  confirmingLabel,
  cancelLabel,
  destructive = true,
  confirmText,
  onConfirm,
  children,
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  const inputId = useId();
  const [internalOpen, setInternalOpen] = useState(false);
  const [typed, setTyped] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const isOpen = open ?? internalOpen;

  const setOpen = (next: boolean) => {
    if (pending && !next) {
      return;
    }
    if (!next) {
      setTyped('');
      setError(null);
    }
    setInternalOpen(next);
    onOpenChange?.(next);
  };

  const matches = confirmText === undefined || typed === confirmText;

  const confirm = async () => {
    if (!matches || pending) {
      return;
    }
    setPending(true);
    setError(null);
    try {
      await onConfirm();
      setPending(false);
      setOpen(false);
    } catch (cause) {
      setPending(false);
      setError(cause);
    }
  };

  return (
    <AlertDialog open={isOpen} onOpenChange={setOpen}>
      {trigger ? <AlertDialogTrigger asChild>{trigger}</AlertDialogTrigger> : null}
      <AlertDialogContent
        title={title}
        description={description}
        footer={
          <>
            <AlertDialogCancel asChild>
              <Button variant="ghost" disabled={pending}>
                {cancelLabel ?? t('acoes.cancelar')}
              </Button>
            </AlertDialogCancel>
            <Button
              variant={destructive ? 'danger' : 'primary'}
              disabled={!matches}
              loading={pending}
              onClick={() => {
                void confirm();
              }}
            >
              {pending && confirmingLabel ? confirmingLabel : confirmLabel}
            </Button>
          </>
        }
      >
        {children || confirmText !== undefined || error !== null ? (
          <>
            {children}
            {confirmText !== undefined ? (
              <div className="field">
                <label className="field__label" htmlFor={inputId}>
                  <Trans
                    i18nKey="confirmar.digite"
                    values={{ texto: confirmText }}
                    components={{ code: <code /> }}
                  />
                </label>
                <input
                  id={inputId}
                  className="input"
                  type="text"
                  autoComplete="off"
                  spellCheck={false}
                  value={typed}
                  aria-describedby={typed && !matches ? `${inputId}-err` : undefined}
                  aria-invalid={typed !== '' && !matches ? true : undefined}
                  onChange={(event) => {
                    setTyped(event.target.value);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      event.preventDefault();
                      void confirm();
                    }
                  }}
                />
                {typed && !matches ? (
                  <div className="field__hint" id={`${inputId}-err`}>
                    {t('confirmar.naoConfere')}
                  </div>
                ) : null}
              </div>
            ) : null}
            {error !== null ? <ErrorPanel error={error} compact /> : null}
          </>
        ) : null}
      </AlertDialogContent>
    </AlertDialog>
  );
}
