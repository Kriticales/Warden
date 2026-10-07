/**
 * "2. Formato" (SPEC T19): pasta packwiz ou arquivo .zip do pack packwiz. Os formatos de
 * outros launchers (P1) chegam com a E-02 em `features/export/formats/`.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { ExportFormat } from '../../../lib/ipc/bindings';
import { Choice } from '../../settings/components/fields';

const FORMATS: readonly { value: ExportFormat; key: 'pasta' | 'zip' }[] = [
  { value: 'folder', key: 'pasta' },
  { value: 'zip', key: 'zip' },
];

export function FormatPanel({
  value,
  onChange,
}: {
  value: ExportFormat;
  onChange: (format: ExportFormat) => void;
}) {
  const { t } = useTranslation('exportar');
  const name = useId();
  return (
    <section className="panel" aria-labelledby="ex-formato">
      <h2 className="panel__title panel__title--sans" id="ex-formato">
        {t('formato.titulo')}
      </h2>
      <fieldset className="m-0 mt-3 border-0 p-0">
        <legend className="sr-only">{t('formato.titulo')}</legend>
        <div className="choice-list choice-list--2">
          {FORMATS.map((format) => (
            <Choice
              key={format.value}
              name={name}
              title={t(`formato.${format.key}.titulo`)}
              desc={t(`formato.${format.key}.desc`)}
              checked={value === format.value}
              onSelect={() => {
                onChange(format.value);
              }}
            />
          ))}
        </div>
      </fieldset>
    </section>
  );
}
