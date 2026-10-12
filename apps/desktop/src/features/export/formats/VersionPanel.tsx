/** A versão do pack, pedida quando o `pack.toml` não tem (o formato exige). */
import { useTranslation } from 'react-i18next';

import { TextField } from '../../settings/components/fields';
import { MAX_VERSION_CHARS, versionValid } from './model';

export function VersionPanel({
  value,
  onChange,
}: {
  value: string;
  onChange: (version: string) => void;
}) {
  const { t } = useTranslation('exportarFormatos');
  const invalid = value !== '' && !versionValid(value);
  return (
    <section className="panel" aria-labelledby="exf-versao">
      <h2 className="panel__title panel__title--sans" id="exf-versao">
        {t('versao.titulo')}
      </h2>
      <p className="t-sm t-2 mt-2">{t('versao.texto')}</p>
      <div className="mt-3" style={{ maxWidth: 280 }}>
        <TextField
          label={t('versao.rotulo')}
          hint={t('versao.dica')}
          error={invalid ? t('versao.erro') : undefined}
          value={value}
          maxLength={MAX_VERSION_CHARS * 2}
          mono
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      </div>
    </section>
  );
}
