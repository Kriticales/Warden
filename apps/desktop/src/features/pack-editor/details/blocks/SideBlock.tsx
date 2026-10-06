/** Lado do item, editável (SPEC T07): muda o `.pw.toml` na hora, pela mesma escrita da lista. */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { useSetSide } from '../../api';
import { SIDE_CHOICES, UNKNOWN_SIDE } from '../../mods/ModRow';
import type { DetailBlockProps } from '../blocks';

export function SideBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  const id = useId();
  const setSide = useSetSide(packId);
  if (item.state === 'invalid') {
    return null;
  }
  if (!item.sideEditable) {
    return (
      <div className="field">
        <span className="field__label">{t('detalhes.lado')}</span>
        <p className="t-sm">{t(`mods.lado.${item.side}`)}</p>
        <div className="field__hint">{t('detalhes.ladoNaoEditavel')}</div>
      </div>
    );
  }
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {t('detalhes.lado')}
      </label>
      <select
        id={id}
        className="select"
        value={item.side}
        disabled={setSide.isPending}
        aria-describedby={`${id}-hint`}
        onChange={(event) => {
          const side = SIDE_CHOICES.find((choice) => choice === event.target.value);
          if (!side) return;
          setSide.mutate({ paths: [item.path], side });
        }}
      >
        {item.side === 'unknown' ? (
          <option value={UNKNOWN_SIDE} disabled>
            {t('mods.lado.unknown')}
          </option>
        ) : null}
        {SIDE_CHOICES.map((side) => (
          <option key={side} value={side}>
            {t(`mods.lado.${side}`)}
          </option>
        ))}
      </select>
      {setSide.isError ? <ErrorPanel compact error={setSide.error} /> : null}
      <div className="field__hint" id={`${id}-hint`}>
        {t('detalhes.ladoDica')}
      </div>
    </div>
  );
}
