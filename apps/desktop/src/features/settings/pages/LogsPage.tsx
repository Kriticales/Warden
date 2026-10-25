/**
 * Configurações → Privacidade e registros (T21): nível de detalhe dos registros (vale na hora)
 * e a pasta onde eles ficam.
 */
import { FolderOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { LogLevel } from '../../../lib/ipc/bindings';
import { useRevealLogsFolder, useSettings } from '../api';
import { RowSelect, SettingRow, SettingsFrame } from '../layout/SettingsFrame';
import { useSaveSetting } from '../layout/save';

const LEVELS: readonly LogLevel[] = ['normal', 'detailed'];
const DEFAULT_LEVEL: LogLevel = 'normal';

export function LogsPage() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const { save, pending } = useSaveSetting();
  const reveal = useRevealLogsFolder();
  return (
    <SettingsFrame page="registros">
      <div className="sform">
        <SettingRow title={t('registros.nivel')} desc={t('registros.nivelDesc')}>
          <RowSelect
            value={settings.data?.logLevel ?? DEFAULT_LEVEL}
            options={LEVELS.map((level) => ({
              value: level,
              label: t(`registros.niveis.${level}`),
            }))}
            disabled={pending}
            onChange={(value) => {
              const level = LEVELS.find((option) => option === value);
              if (level) {
                save({ logLevel: level });
              }
            }}
          />
        </SettingRow>
        <SettingRow title={t('registros.pasta')} desc={t('registros.pastaDesc')}>
          <div className="stack-2">
            <div>
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
          </div>
        </SettingRow>
      </div>
      <div className="sform sform--note">
        <Alert kind="ok" role="none">
          {t('registros.dica')}
        </Alert>
      </div>
    </SettingsFrame>
  );
}
