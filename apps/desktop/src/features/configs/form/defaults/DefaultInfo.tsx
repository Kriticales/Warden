/**
 * O padrão de uma chave no formulário (protótipo `configs-formulario`; `cfgRow` do design
 * system): "padrão: X · origem", "deduzido" quando veio de uma frase do comentário e "padrão
 * desconhecido" quando o arquivo não informa (nunca um padrão inventado); a marca "alterado";
 * o erro de faixa do valor que já está no arquivo; e o botão **Restaurar padrão**.
 */
import { CircleAlert, Pencil, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../../components/ui/button';
import { Icon } from '../../../../components/ui/icon';
import type { ConfigField } from '../../../../lib/ipc/bindings';
import { formatNumber, rangeText } from '../model';
import { canRestore, isChanged, knownDefault } from './model';

/** "padrão: X" com a origem, ou "padrão desconhecido". */
export function DefaultLine({ field }: { field: ConfigField }) {
  const { t } = useTranslation('configsPadrao');
  const value = field.default;
  if (!value) {
    return (
      <div className="cfgform__def t-3" title={t('campo.desconhecidoDica')}>
        {t('campo.desconhecido')}
      </div>
    );
  }
  const origin = t(`campo.origem_${value.source}`);
  return (
    <div className="cfgform__def" title={origin}>
      {t('campo.padrao')} <span className="t-mono">{value.value}</span>{' '}
      <span className="t-3">· {origin}</span>
      {value.inferred ? (
        <>
          {' '}
          <span className="tag tag--plain" title={t('campo.deduzidoDica')}>
            {t('campo.deduzido')}
          </span>
        </>
      ) : null}
    </div>
  );
}

/** A marca "alterado" ao lado do nome. */
export function ChangedMark({ field }: { field: ConfigField }) {
  const { t } = useTranslation('configsPadrao');
  if (!isChanged(field)) return null;
  return (
    <span className="cfgform__mark" title={t('campo.alteradoDica')}>
      <Icon icon={Pencil} size="sm" />
      {t('campo.alterado')}
    </span>
  );
}

/** A frase do valor do arquivo fora da faixa ou dos valores aceitos. */
function RangeMessage({ field }: { field: ConfigField }) {
  const { t } = useTranslation('configsPadrao');
  const def = knownDefault(field);
  const valor =
    field.value.type === 'list' ? field.value.value.join(', ') : String(field.value.value);
  if (field.range && !field.range.inferred) {
    const range = rangeText(field.range);
    const faixa =
      range.kind === 'both'
        ? t('faixa.ambos', { min: range.min, max: range.max })
        : range.kind === 'min'
          ? t('faixa.min', { min: range.min })
          : range.kind === 'max'
            ? t('faixa.max', { max: range.max })
            : range.text;
    const number = Number(valor);
    const shown = valor.trim() !== '' && Number.isFinite(number) ? formatNumber(number) : valor;
    return (
      <>
        {def === null
          ? t('campo.foraDaFaixaSemPadrao', { valor: shown, faixa })
          : t('campo.foraDaFaixa', { valor: shown, faixa, padrao: def })}
      </>
    );
  }
  return (
    <>
      {def === null
        ? t('campo.foraDosValoresSemPadrao', { valor })
        : t('campo.foraDosValores', { valor, padrao: def })}
    </>
  );
}

/** O erro do valor que já está no arquivo fora do permitido. */
export function RangeError({ field, id }: { field: ConfigField; id: string }) {
  if (!field.outOfRange) return null;
  return (
    <div className="field__error" id={id} role="status">
      <Icon icon={CircleAlert} size="sm" />
      <span>
        <RangeMessage field={field} />
      </span>
    </div>
  );
}

export interface RestoreButtonProps {
  field: ConfigField;
  onRestore: (field: ConfigField) => void;
}

/** "Restaurar padrão" da chave (só quando ela está diferente de um padrão conhecido). */
export function RestoreButton({ field, onRestore }: RestoreButtonProps) {
  const { t } = useTranslation('configsPadrao');
  if (!canRestore(field)) return null;
  return (
    <Button
      size="sm"
      variant="ghost"
      iconOnly
      icon={RotateCcw}
      onClick={() => {
        onRestore(field);
      }}
    >
      {t('campo.restaurarChave', { chave: field.key })}
    </Button>
  );
}
