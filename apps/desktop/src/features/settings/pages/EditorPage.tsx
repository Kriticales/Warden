/** Configurações → Editor de configs (T21): mostrar as diferenças antes de salvar. */
import { Trans, useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { useSettings } from '../api';
import { RowSwitch, SettingRow, SettingsFrame } from '../layout/SettingsFrame';
import { useSaveSetting } from '../layout/save';

export function EditorPage() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const { save, pending } = useSaveSetting();
  return (
    <SettingsFrame page="editor">
      <div className="sform">
        <SettingRow title={t('editor.diferencas')} desc={t('editor.diferencasDesc')}>
          <RowSwitch
            checked={settings.data?.configDiffBeforeSave ?? true}
            disabled={pending}
            onChange={(checked) => {
              save({ configDiffBeforeSave: checked });
            }}
          />
        </SettingRow>
      </div>
      <div className="sform sform--note">
        <Alert kind="neutral" role="none">
          <Trans
            t={t}
            i18nKey="editor.nota"
            components={{ b: <b />, mono: <span className="t-mono" /> }}
          />
        </Alert>
      </div>
    </SettingsFrame>
  );
}
