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
import { FileCode, Search } from 'lucide-react';
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
  FormItem,
  FormValue,
  KeyDifference,
  PackId,
  TextOnlyReason,
} from '../../../lib/ipc/bindings';
import { fold } from '../model';
import { FilterChip } from '../places/FilterChip';
import { useConfigForm, useFormApply } from './api';
import { ChangedFilter } from './defaults/ChangedFilter';
import { changedCount, onlyChanged } from './defaults/model';
import { FieldRow, type PackKey } from './FieldRow';
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
  /** As chaves diferentes entre o teste local e o pack (SPEC T12; ADR-0067). */
  pack?: FormPackDiff | null;
}

/** As diferenças com o outro lado (o pack, se o editor mostra o teste; ou o contrário). */
export interface FormPackDiff {
  keys: ReadonlyMap<string, KeyDifference>;
  side: 'test' | 'pack';
  /** "Trazer para o pack" de uma chave. */
  onBring?: (key: string) => void;
  /** A chave sendo trazida agora. */
  busyKey?: string | null;
}

/** O texto em que a busca do arquivo procura: chave, rótulo do mod e descrição. */
function searchable(item: FormItem): string {
  if (item.kind !== 'field') return fold(item.key);
  return fold(
    [item.key, item.label?.text ?? '', item.label?.tooltip ?? '', item.description ?? ''].join(' '),
  );
}

/**
 * Os itens que passam nos filtros. Uma seção só aparece se algum campo dela aparece; sem filtro
 * nenhum, tudo.
 */
export function visibleItems(
  items: readonly FormItem[],
  {
    query,
    changedOnly,
    diffKeys,
  }: {
    query: string;
    changedOnly: boolean;
    diffKeys: ReadonlySet<string> | null;
  },
): FormItem[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0 && !changedOnly && diffKeys === null) return [...items];
  const base = changedOnly ? onlyChanged(items) : items;
  const out: FormItem[] = [];
  let section: FormItem | null = null;
  for (const item of base) {
    if (item.kind === 'section') {
      section = item;
      continue;
    }
    const text = searchable(item);
    const passes =
      words.every((word) => text.includes(word)) && (diffKeys === null || diffKeys.has(item.key));
    if (!passes) continue;
    if (section) {
      out.push(section);
      section = null;
    }
    out.push(item);
  }
  return out;
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
  pack = null,
}: ConfigFormViewProps) {
  const { t } = useTranslation('configsFormulario');
  const { t: tp } = useTranslation('configsPadrao');
  const { t: tc } = useTranslation('configs');
  const [changedOnly, setChangedOnly] = useState(false);
  const [diffOnly, setDiffOnly] = useState(false);
  const [query, setQuery] = useState('');
  const changed = changedCount(form.items);
  const diffCount = pack ? pack.keys.size : 0;
  const items = visibleItems(form.items, {
    query,
    changedOnly,
    diffKeys: diffOnly && pack ? new Set(pack.keys.keys()) : null,
  });
  const filtering = query.trim() !== '' || changedOnly || (diffOnly && diffCount > 0);
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

  const packKey = (key: string): PackKey | null => {
    const difference = pack?.keys.get(key);
    if (!pack || !difference) return null;
    return {
      difference,
      side: pack.side,
      onBring: pack.onBring
        ? () => {
            pack.onBring?.(key);
          }
        : undefined,
      busy: pack.busyKey === key,
    };
  };

  return (
    <div className="cfgform cfg-form" role="group" aria-label={t('area', { arquivo: path })}>
      <div className="cfgform__tools" role="group" aria-label={tc('filtros.rotulo')}>
        <label className="qbox qbox--sm">
          <Search className="icon icon--sm" aria-hidden="true" />
          <span className="sr-only">{t('buscarNoArquivo')}</span>
          <input
            className="input input--sm"
            type="search"
            value={query}
            placeholder={t('buscarNoArquivo')}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </label>
        <ChangedFilter checked={changedOnly} onChange={setChangedOnly} count={changed} />
        {diffCount > 0 ? (
          <FilterChip
            label={tc('filtros.difereCurto')}
            place="test"
            pressed={diffOnly}
            count={diffCount}
            title={tc('filtros.difereDica')}
            onChange={setDiffOnly}
          />
        ) : null}
      </div>
      {changedOnly && changed === 0 ? (
        <p className="t-sm t-3 cfg-changed-filter__empty">{tp('filtro.nenhumaNoArquivo')}</p>
      ) : filtering && items.length === 0 ? (
        <p className="t-sm t-3 cfg-changed-filter__empty">{t('nenhumaComFiltro')}</p>
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
                pack={packKey(item.key)}
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
