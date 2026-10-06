/**
 * Etapa 1, "Nome e pasta" (T03): nome (1–64), autor (padrão: o nome do jogador), descrição
 * opcional e a pasta. A pasta acompanha o nome (`<pasta dos packs>\<nome>`) até o usuário
 * digitar ou escolher outra. Cada mudança é conferida no Rust (`pack_create_check`), sem
 * escrever nada: pasta com arquivos é recusada aqui, antes de qualquer escrita (CA-T03-04).
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { CircleAlert, FolderOpen } from 'lucide-react';
import { useEffect, useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { Button } from '../../../../components/ui/button';
import { Icon } from '../../../../components/ui/icon';
import { showToast } from '../../../../components/ui/toast';
import { appErrorMessage, toAppError } from '../../../../lib/ipc/errors';
import { commandError } from '../../../../lib/ipc/query';
import { createCheckQuery, useChooseFolder, useCreateDefaults } from '../../api';
import { checkProblem, type CheckProblem, type Draft } from './draft';

/** Espera entre digitar e conferir a pasta. */
export const CHECK_DELAY_MS = 250;

function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebounced(value);
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [value, delayMs]);
  return debounced;
}

export interface NameStepProps {
  draft: Draft;
  onChange: (patch: Partial<Draft>) => void;
  /** Erro da conferência feita ao clicar em Próximo (ou ao criar). */
  submitError: unknown;
}

export function NameStep({ draft, onChange, submitError }: NameStepProps) {
  const { t } = useTranslation('packs');
  const ids = { name: useId(), author: useId(), description: useId(), path: useId() };
  const defaults = useCreateDefaults();
  const choose = useChooseFolder();
  const checked = useDebounced(
    { name: draft.name, destination: draft.destination },
    CHECK_DELAY_MS,
  );
  const check = useQuery({
    ...createCheckQuery(checked.name, checked.destination),
    enabled: checked.name.trim() !== '',
    placeholderData: keepPreviousData,
  });

  const liveError = check.isError && draft.name.trim() !== '' ? check.error : null;
  const problem: CheckProblem | null =
    checkProblem(commandError(submitError), draft.name) ??
    checkProblem(commandError(liveError), draft.name);
  const otherError = submitError !== null && problem === null ? submitError : null;

  // Pasta automática: a que o Rust calculou (ou a que ele recusou, com o motivo ao lado).
  const autoPath = check.isSuccess ? check.data : (commandError(check.error)?.params.path ?? '');
  const shownPath = draft.destination ?? (draft.name.trim() === '' ? '' : autoPath);

  const pickFolder = () => {
    choose.mutate('createDestination', {
      onSuccess: (path) => {
        if (path !== null) onChange({ destination: path });
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  return (
    <div className="wizard__body stack">
      <Field
        id={ids.name}
        label={t('criar.nome.rotulo')}
        hint={t('criar.nome.dica')}
        error={problem?.field === 'name' ? t(`criar.${problem.key}`) : null}
      >
        {(describedBy, invalid) => (
          <input
            id={ids.name}
            className="input"
            type="text"
            autoComplete="off"
            required
            maxLength={128}
            value={draft.name}
            aria-invalid={invalid || undefined}
            aria-describedby={describedBy}
            onChange={(event) => {
              onChange({ name: event.target.value });
            }}
          />
        )}
      </Field>
      <Field
        id={ids.author}
        label={t('criar.autor.rotulo')}
        hint={defaults.data ? t('criar.autor.dica', { padrao: defaults.data.author }) : null}
      >
        {(describedBy) => (
          <input
            id={ids.author}
            className="input"
            type="text"
            autoComplete="off"
            maxLength={256}
            value={draft.author}
            placeholder={defaults.data?.author}
            aria-describedby={describedBy}
            onChange={(event) => {
              onChange({ author: event.target.value });
            }}
          />
        )}
      </Field>
      <Field
        id={ids.description}
        label={
          <>
            {t('criar.descricao.rotulo')}{' '}
            <span className="opt">{t('criar.descricao.opcional')}</span>
          </>
        }
      >
        {(describedBy) => (
          <input
            id={ids.description}
            className="input"
            type="text"
            autoComplete="off"
            maxLength={4096}
            value={draft.description}
            placeholder={t('criar.descricao.placeholder')}
            aria-describedby={describedBy}
            onChange={(event) => {
              onChange({ description: event.target.value });
            }}
          />
        )}
      </Field>
      <Field
        id={ids.path}
        label={t('criar.pasta.rotulo')}
        hint={t('criar.pasta.dica')}
        error={problem?.field === 'path' ? t(`criar.${problem.key}`) : null}
      >
        {(describedBy, invalid) => (
          <>
            <div className="field__row">
              <input
                id={ids.path}
                className="input input--mono"
                type="text"
                autoComplete="off"
                spellCheck={false}
                value={shownPath}
                aria-invalid={invalid || undefined}
                aria-describedby={describedBy}
                onChange={(event) => {
                  onChange({ destination: event.target.value === '' ? null : event.target.value });
                }}
              />
              <Button icon={FolderOpen} loading={choose.isPending} onClick={pickFolder}>
                {t('criar.pasta.escolher')}
              </Button>
            </div>
            {draft.destination !== null ? (
              <div>
                <Button
                  variant="link"
                  size="sm"
                  onClick={() => {
                    onChange({ destination: null });
                  }}
                >
                  {t('criar.pasta.padrao')}
                </Button>
              </div>
            ) : null}
          </>
        )}
      </Field>
      {otherError !== null ? <ErrorPanel compact error={otherError} /> : null}
    </div>
  );
}

/** Campo com rótulo, dica e erro ligados por `aria-describedby` (DESIGN-SYSTEM §6). */
function Field({
  id,
  label,
  hint,
  error,
  children,
}: {
  id: string;
  label: ReactNode;
  hint?: string | null;
  error?: string | null;
  children: (describedBy: string | undefined, invalid: boolean) => ReactNode;
}) {
  const hintId = `${id}-dica`;
  const errorId = `${id}-erro`;
  const describedBy =
    [error ? errorId : null, hint ? hintId : null].filter(Boolean).join(' ') || undefined;
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      {children(describedBy, Boolean(error))}
      {error ? (
        <div className="field__error" id={errorId}>
          <Icon icon={CircleAlert} size="sm" />
          <span>{error}</span>
        </div>
      ) : null}
      {hint ? (
        <p className="field__hint" id={hintId}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}
