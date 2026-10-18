/**
 * "2. Formato" (SPEC T19): pasta packwiz, arquivo .zip do pack packwiz e, na v1 (P1), os
 * formatos de outros launchers (.mrpack e zip da CurseForge; E-02, `features/export/formats/`),
 * a instância pronta para o Prism (E-04, `features/export/prism/`) e o pacote para servidor (E-03,
 * `features/export/server-pack/`).
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Choice } from '../../settings/components/fields';
import type { UiFormat } from '../model';

export function FormatPanel({
  value,
  onChange,
}: {
  value: UiFormat;
  onChange: (format: UiFormat) => void;
}) {
  const { t } = useTranslation([
    'exportar',
    'exportarFormatos',
    'instanciaPrism',
    'pacoteServidor',
  ]);
  const name = useId();
  const choice = (format: UiFormat, title: string, desc: string) => (
    <Choice
      key={format}
      name={name}
      title={title}
      desc={desc}
      checked={value === format}
      onSelect={() => {
        onChange(format);
      }}
    />
  );
  return (
    <section className="panel" aria-labelledby="ex-formato">
      <h2 className="panel__title panel__title--sans" id="ex-formato">
        {t('exportar:formato.titulo')}
      </h2>
      <fieldset className="m-0 mt-3 border-0 p-0" aria-labelledby="ex-formato">
        <div className="choice-list choice-list--2">
          {choice('folder', t('exportar:formato.pasta.titulo'), t('exportar:formato.pasta.desc'))}
          {choice('zip', t('exportar:formato.zip.titulo'), t('exportar:formato.zip.desc'))}
          {choice(
            'mrpack',
            t('exportarFormatos:formato.mrpack.titulo'),
            t('exportarFormatos:formato.mrpack.desc'),
          )}
          {choice(
            'curseforge',
            t('exportarFormatos:formato.curseforge.titulo'),
            t('exportarFormatos:formato.curseforge.desc'),
          )}
          {choice('prism', t('instanciaPrism:formato.titulo'), t('instanciaPrism:formato.desc'))}
          {choice('server', t('pacoteServidor:formato.titulo'), t('pacoteServidor:formato.desc'))}
        </div>
      </fieldset>
    </section>
  );
}
