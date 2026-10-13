/**
 * Uma linha do formulário (protótipo `configs-formulario`; `.cfgform__row` do design system):
 * à esquerda o rótulo do mod (quando o jar tem), a chave real, a descrição, a faixa e o padrão
 * (com "deduzido" quando veio de uma frase do comentário); à direita o controle do tipo.
 *
 * Texto e números são aplicados ao sair do campo ou com Enter; liga/desliga, opções e cor, na
 * hora. Número que não serve ou fora da faixa que o arquivo declara fica com o erro no próprio
 * campo e não é aplicado.
 */
import { CircleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { ConfigField, FormValue } from '../../../lib/ipc/bindings';
import { Switch } from '../../settings/components/fields';
import {
  checkNumber,
  fieldName,
  rangeText,
  sameValue,
  valueFromText,
  valueText,
  type NumberProblem,
} from './model';

export interface FieldRowProps {
  field: ConfigField;
  onEdit: (field: ConfigField, value: FormValue) => void;
}

function Meta({ field, id }: { field: ConfigField; id: string }) {
  const { t } = useTranslation('configsFormulario');
  const range = field.range ? rangeText(field.range) : null;
  return (
    <>
      {field.label ? (
        <div
          className="cfgform__label"
          title={t('campo.rotuloDica', {
            idioma: t(field.label.locale === 'ptBr' ? 'campo.idioma_ptBr' : 'campo.idioma_enUs'),
          })}
        >
          {field.label.text}
        </div>
      ) : null}
      <div className="cfgform__key">{field.key}</div>
      <div className="cfgform__desc" id={`${id}-desc`}>
        {field.label?.tooltip ? <p>{field.label.tooltip}</p> : null}
        {field.description ? <p className="cfgform__comment">{field.description}</p> : null}
        {range ? (
          <p>
            {range.kind === 'both'
              ? t('campo.faixa', { min: range.min, max: range.max })
              : range.kind === 'min'
                ? t('campo.faixaMin', { min: range.min })
                : range.kind === 'max'
                  ? t('campo.faixaMax', { max: range.max })
                  : t('campo.faixaTexto', { texto: range.text })}
            {field.range?.inferred ? (
              <>
                {' '}
                <span className="tag tag--plain" title={t('campo.deduzidoDica')}>
                  {t('campo.deduzido')}
                </span>
              </>
            ) : null}
          </p>
        ) : null}
      </div>
      {field.default ? (
        <div className="cfgform__def">
          {t('campo.padrao', { valor: field.default.value })}
          {field.default.inferred ? (
            <>
              {' '}
              <span className="tag tag--plain" title={t('campo.deduzidoDica')}>
                {t('campo.deduzido')}
              </span>
            </>
          ) : null}
        </div>
      ) : null}
    </>
  );
}

export function FieldRow({ field, onEdit }: FieldRowProps) {
  const { t } = useTranslation('configsFormulario');
  const id = useId();
  const name = fieldName(field);
  const current = valueText(field.value);
  const [draft, setDraft] = useState(current);
  const [problem, setProblem] = useState<NumberProblem | null>(null);
  // O valor do arquivo mudou (aplicado, desfeito ou relido): o rascunho acompanha.
  const [seen, setSeen] = useState(current);
  if (seen !== current) {
    setSeen(current);
    setDraft(current);
    setProblem(null);
  }

  const submit = (text: string) => {
    // Sair do campo sem mudar nada não regrava (uma lista com `""` viraria lista vazia).
    if (text === current) {
      setProblem(null);
      return;
    }
    const control = field.control;
    if (control.type === 'number') {
      const found = checkNumber(text, control.integer, field.range);
      setProblem(found);
      if (found) return;
    }
    const value = valueFromText(field, text);
    if (!sameValue(value, field.value)) {
      onEdit(field, value);
    }
  };

  const errorId = `${id}-err`;
  const described = [problem ? errorId : '', `${id}-desc`].filter(Boolean).join(' ');
  const control = field.control;
  let input;
  switch (control.type) {
    case 'toggle': {
      const checked = current === 'true';
      input = (
        <Switch
          label={<span className="sr-only">{name}</span>}
          checked={checked}
          onText={t('campo.ligado')}
          offText={t('campo.desligado')}
          onChange={(next) => {
            submit(String(next));
          }}
        />
      );
      break;
    }
    case 'options':
      input = (
        <select
          className="select select--sm"
          aria-label={name}
          aria-describedby={described}
          value={current}
          onChange={(event) => {
            submit(event.target.value);
          }}
        >
          {control.values.includes(current) ? null : <option value={current}>{current}</option>}
          {control.values.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      );
      break;
    case 'list':
      input = (
        <textarea
          className="textarea input--sm t-mono"
          aria-label={name}
          aria-describedby={`${described} ${id}-list`}
          rows={Math.min(Math.max(draft.split('\n').length, 2), 8)}
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={() => {
            submit(draft);
          }}
        />
      );
      break;
    case 'color':
      input = (
        <span className="row cfgform__color">
          <input
            type="color"
            aria-label={t('campo.cor', { campo: name })}
            value={current.toLowerCase()}
            onChange={(event) => {
              // Mantém maiúsculas ou minúsculas como o arquivo escreve.
              const upper = current !== current.toLowerCase();
              submit(upper ? event.target.value.toUpperCase() : event.target.value);
            }}
          />
          <TextInput
            name={name}
            described={described}
            draft={draft}
            invalid={false}
            onDraft={setDraft}
            onSubmit={submit}
          />
        </span>
      );
      break;
    case 'number':
    case 'text':
      input = (
        <TextInput
          name={name}
          described={described}
          draft={draft}
          invalid={problem !== null}
          numeric={control.type === 'number'}
          onDraft={setDraft}
          onSubmit={submit}
        />
      );
      break;
  }

  return (
    <div className={problem ? 'cfgform__row cfgform__row--error' : 'cfgform__row'}>
      <div className="cfgform__info">
        <Meta field={field} id={id} />
        {control.type === 'list' ? (
          <div className="cfgform__desc" id={`${id}-list`}>
            {t('campo.lista')}
          </div>
        ) : null}
        {problem ? (
          <div className="field__error" id={errorId} role="alert">
            <Icon icon={CircleAlert} size="sm" />
            <span>
              {problem === 'foraDaFaixa'
                ? t('campo.foraDaFaixa', { valor: draft.trim() })
                : t(`campo.${problem}`)}
            </span>
          </div>
        ) : null}
      </div>
      <div className="cfgform__ctl">{input}</div>
    </div>
  );
}

interface TextInputProps {
  name: string;
  described: string;
  draft: string;
  invalid: boolean;
  numeric?: boolean;
  onDraft: (text: string) => void;
  onSubmit: (text: string) => void;
}

function TextInput({
  name,
  described,
  draft,
  invalid,
  numeric = false,
  onDraft,
  onSubmit,
}: TextInputProps) {
  return (
    <input
      className="input input--sm"
      type="text"
      inputMode={numeric ? 'decimal' : undefined}
      aria-label={name}
      aria-describedby={described}
      aria-invalid={invalid ? true : undefined}
      value={draft}
      onChange={(event) => {
        onDraft(event.target.value);
      }}
      onBlur={() => {
        onSubmit(draft);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          onSubmit(draft);
        }
      }}
    />
  );
}
