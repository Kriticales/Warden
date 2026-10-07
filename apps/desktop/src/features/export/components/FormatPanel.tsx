/**
 * "2. Formato" (SPEC T19): pasta packwiz ou arquivo .zip do pack packwiz. Os formatos de
 * outros launchers (P1) chegam com a E-02 em `features/export/formats/`.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { ExportFormat } from '../../../lib/ipc/bindings';
import { Choice } from '../../settings/components/fields';

/** Os formatos do `export_run` e a instância pronta para o Prism (comando próprio, E-04). */
export type ExportChoice = ExportFormat | 'prism';

const FORMATS: readonly { value: ExportChoice; key: 'pasta' | 'zip' | 'prism' }[] = [
  { value: 'folder', key: 'pasta' },
  { value: 'zip', key: 'zip' },
  { value: 'prism', key: 'prism' },
];

export function FormatPanel({
  value,
  onChange,
}: {
  value: ExportChoice;
  onChange: (format: ExportChoice) => void;
}) {
  const { t } = useTranslation('exportar');
  const { t: tPrism } = useTranslation('instanciaPrism');
  const name = useId();
  return (
    <section className="panel" aria-labelledby="ex-formato">
      <h2 className="panel__title panel__title--sans" id="ex-formato">
        {t('formato.titulo')}
      </h2>
      <fieldset className="m-0 mt-3 border-0 p-0" aria-labelledby="ex-formato">
        <div className="choice-list choice-list--2">
          {FORMATS.map((format) => (
            <Choice
              key={format.value}
              name={name}
              title={
                format.key === 'prism'
                  ? tPrism('formato.titulo')
                  : t(`formato.${format.key}.titulo`)
              }
              desc={
                format.key === 'prism' ? tPrism('formato.desc') : t(`formato.${format.key}.desc`)
              }
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
