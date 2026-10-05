/**
 * Privacidade e registros (T21): nível de detalhe dos registros (vale na hora) e a pasta onde
 * eles ficam.
 */
import { FolderOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import type { LogLevel } from '../../../lib/ipc/bindings';
import { useRevealLogsFolder, useSettings } from '../api';
import { SelectField } from './fields';
import { SettingsPanel, useSaveSetting } from './SettingsPanel';

const LEVELS: readonly LogLevel[] = ['normal', 'detailed'];
const DEFAULT_LEVEL: LogLevel = 'normal';

export function LogsSection() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const { save, pending } = useSaveSetting();
  const reveal = useRevealLogsFolder();
  return (
    <SettingsPanel title={t('registros.titulo')}>
      <div className="row row--wrap row--gap-4 items-end">
        <SelectField
          label={t('registros.nivel')}
          options={LEVELS.map((level) => ({ value: level, label: t(`registros.niveis.${level}`) }))}
          value={settings.data?.logLevel ?? DEFAULT_LEVEL}
          disabled={pending}
          onChange={(event) => {
            const level = LEVELS.find((option) => option === event.target.value);
            if (level) {
              save({ logLevel: level });
            }
          }}
        />
        <Button
          icon={FolderOpen}
          loading={reveal.isPending}
          onClick={() => {
            reveal.mutate(undefined);
          }}
        >
          {t('registros.abrirPasta')}
        </Button>
      </div>
      {reveal.isError ? <ErrorPanel compact error={reveal.error} /> : null}
      <p className="field__hint">{t('registros.dica')}</p>
    </SettingsPanel>
  );
}
