/**
 * O modo formulário do editor de configs (SPEC T12, parte P1; C-04).
 *
 * O formulário é outra forma de ver o **mesmo texto** do editor: cada mudança vai ao Rust
 * (`config_form_apply`), que devolve o texto com só os bytes do valor trocados, e esse texto
 * passa a ser o do editor. Salvar, desfazer, a diferença antes de gravar e o aviso de alteração
 * externa são os de sempre. Se o Rust não confirmar a edição (a releitura ou a comparação byte
 * a byte falhou), nada muda e o editor cai para o texto com o motivo (ADR-0054).
 *
 * Aqui ficam: o hook com o estado do formulário do arquivo, a alternância Formulário/Texto, a
 * lista de campos e os avisos de "só texto". O filtro "Só o que mudou do padrão" e
 * "Restaurar padrão" são da C-06 (`./defaults/`).
 */
import { FileCode } from 'lucide-react';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  ConfigField,
  ConfigForm as ConfigFormData,
  FormValue,
  PackId,
  TextOnlyReason,
} from '../../../lib/ipc/bindings';
import { useConfigForm, useFormApply } from './api';
import { ChangedFilter } from './defaults/ChangedFilter';
import { changedCount, onlyChanged } from './defaults/model';
import { FieldRow } from './FieldRow';
import type { EditorMode } from './model';
import './form.css';

export interface FormEditorOptions {
  packId: PackId;
  path: string;
  /** O texto atual do editor (com as alterações não salvas). */
  text: string;
  /** Só arquivos editáveis têm formulário. */
  editable: boolean;
  /** O formulário só é montado no modo formulário (no texto, cada tecla mudaria a chave). */
  active: boolean;
}

/** O estado do formulário do arquivo aberto. */
export type FormEditorState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error'; error: unknown; retry: () => void }
  | { status: 'textOnly'; reason: TextOnlyReason | null }
  | { status: 'ready'; form: ConfigFormData };

export function useFormEditor({
  packId,
  path,
  text,
  editable,
  active,
}: FormEditorOptions): FormEditorState {
  const query = useConfigForm(packId, path, text, editable && active);
  if (!editable) return { status: 'textOnly', reason: null };
  if (!active) return { status: 'idle' };
  if (query.isPending) return { status: 'loading' };
  if (query.isError) {
    return {
      status: 'error',
      error: query.error,
      retry: () => {
        void query.refetch();
      },
    };
  }
  const form = query.data;
  return form.availability.kind === 'ready'
    ? { status: 'ready', form }
    : { status: 'textOnly', reason: form.availability.reason };
}

export interface ModeSwitchProps {
  mode: EditorMode;
  state: FormEditorState;
  onChange: (mode: EditorMode) => void;
}

/** "Formulário | Texto" (protótipo: `segmented`). Formulário indisponível fica com a dica. */
export function ModeSwitch({ mode, state, onChange }: ModeSwitchProps) {
  const { t } = useTranslation('configsFormulario');
  const formDisabled = state.status === 'textOnly';
  const options: { value: EditorMode; label: string; disabled: boolean }[] = [
    { value: 'form', label: t('modo.formulario'), disabled: formDisabled },
    { value: 'text', label: t('modo.texto'), disabled: false },
  ];
  const shown: EditorMode = formDisabled ? 'text' : mode;
  const onKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const next: EditorMode = shown === 'form' ? 'text' : 'form';
    if (next === 'form' && formDisabled) return;
    onChange(next);
    const group = event.currentTarget.parentElement;
    requestAnimationFrame(() => {
      group?.querySelector<HTMLButtonElement>(`[data-value="${next}"]`)?.focus();
    });
  };
  return (
    <span className="segmented" role="radiogroup" aria-label={t('modo.rotulo')}>
      {options.map((option) => {
        const button = (
          <button
            key={option.value}
            type="button"
            role="radio"
            data-value={option.value}
            aria-checked={shown === option.value}
            aria-disabled={option.disabled ? true : undefined}
            tabIndex={shown === option.value ? 0 : -1}
            onKeyDown={onKey}
            onClick={() => {
              if (!option.disabled) onChange(option.value);
            }}
          >
            {option.label}
          </button>
        );
        return option.disabled ? (
          <Tooltip key={option.value} content={t('modo.indisponivel')}>
            {button}
          </Tooltip>
        ) : (
          button
        );
      })}
    </span>
  );
}

/** O motivo de o arquivo abrir só no texto, quando a pessoa está no modo formulário. */
export function TextOnlyNotice({ reason }: { reason: TextOnlyReason }) {
  const { t } = useTranslation('configsFormulario');
  const text =
    reason.code === 'parseFailed'
      ? t('somenteTexto.parseFailed', { line: reason.line, column: reason.column })
      : reason.code === 'notIdentical'
        ? t('somenteTexto.notIdentical', { key: reason.key })
        : t(`somenteTexto.${reason.code}`);
  return (
    <Alert kind="info" compact title={t('somenteTexto.titulo')}>
      {text}
    </Alert>
  );
}

/** A edição pelo formulário não foi confirmada: o arquivo foi para o texto, com o motivo. */
export function FallbackNotice({ error }: { error: unknown }) {
  const { t } = useTranslation('configsFormulario');
  return <ErrorPanel compact title={t('queda.titulo')} error={error} />;
}

export interface ConfigFormViewProps {
  form: ConfigFormData;
  path: string;
  text: string;
  /** O texto novo, com a edição aplicada. */
  onText: (text: string) => void;
  /** Abrir o texto nesta linha ("Editar este trecho como texto"). */
  onOpenText: (line: number) => void;
  /** A edição não foi confirmada: cair para o texto com o erro. */
  onFallback: (error: unknown) => void;
  /** "Restaurar padrão" de uma chave (C-06): o editor mostra a diferença antes. */
  onRestore: (field: ConfigField) => void;
}

/** Os campos do arquivo, na ordem dele. */
export function ConfigFormView({
  form,
  path,
  text,
  onText,
  onOpenText,
  onFallback,
  onRestore,
}: ConfigFormViewProps) {
  const { t } = useTranslation('configsFormulario');
  const { t: tp } = useTranslation('configsPadrao');
  const [changedOnly, setChangedOnly] = useState(false);
  const changed = changedCount(form.items);
  const items = changedOnly ? onlyChanged(form.items) : form.items;
  const apply = useFormApply();
  // As edições vão uma de cada vez, sempre sobre o texto mais novo.
  const latest = useRef(text);
  const queue = useRef<Promise<void>>(Promise.resolve());
  useEffect(() => {
    latest.current = text;
  }, [text]);

  const edit = (field: ConfigField, value: FormValue) => {
    queue.current = queue.current.then(async () => {
      try {
        const next = await apply.mutateAsync({
          path,
          text: latest.current,
          edits: [{ path: field.path, value }],
        });
        latest.current = next;
        onText(next);
      } catch (error) {
        onFallback(error);
      }
    });
  };

  return (
    <div className="cfgform cfg-form" role="group" aria-label={t('area', { arquivo: path })}>
      <ChangedFilter
        checked={changedOnly}
        onChange={setChangedOnly}
        desc={tp('filtro.contagem', { count: changed })}
      />
      {changedOnly && changed === 0 ? (
        <p className="t-sm t-3 cfg-changed-filter__empty">{tp('filtro.nenhumaNoArquivo')}</p>
      ) : null}
      {items.map((item) => {
        switch (item.kind) {
          case 'section':
            return (
              <div key={`s:${item.key}:${String(item.line)}`} className="cfgform__group">
                [{item.key}]
                {item.description ? (
                  <span className="cfgform__desc"> {item.description}</span>
                ) : null}
              </div>
            );
          case 'snippet':
            return (
              <div key={`t:${item.key}:${String(item.line)}`} className="cfgform__row">
                <div className="cfgform__info">
                  <div className="cfgform__key">{item.key}</div>
                  <div className="cfgform__desc">
                    {t(`trecho.${item.reason}`)} ({t('trecho.linha', { linha: item.line })})
                  </div>
                </div>
                <div className="cfgform__ctl">
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={FileCode}
                    aria-label={t('trecho.abrirChave', { chave: item.key })}
                    onClick={() => {
                      onOpenText(item.line);
                    }}
                  >
                    {t('trecho.abrir')}
                  </Button>
                </div>
              </div>
            );
          case 'field':
            return (
              <FieldRow
                key={`f:${item.key}:${String(item.line)}`}
                field={item}
                onEdit={edit}
                onRestore={onRestore}
              />
            );
        }
      })}
    </div>
  );
}

/** O que mostrar enquanto o formulário carrega ou quando falhou. */
export function FormPending({ state }: { state: FormEditorState }) {
  const { t } = useTranslation('configsFormulario');
  if (state.status === 'loading') return <LoadingState label={t('carregando')} />;
  if (state.status === 'error') {
    return <ErrorPanel title={t('erro')} error={state.error} onRetry={state.retry} />;
  }
  return null;
}
