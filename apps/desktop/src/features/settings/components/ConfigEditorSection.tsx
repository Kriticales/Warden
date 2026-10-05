/**
 * Editor de configs (T21): mostrar as diferenças antes de salvar (ligado por padrão).
 */
import { useTranslation } from 'react-i18next';

import { useSettings } from '../api';
import { Checkbox } from './fields';
import { SettingsPanel, useSaveSetting } from './SettingsPanel';

export function ConfigEditorSection() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const { save, pending } = useSaveSetting();
  return (
    <SettingsPanel title={t('editor.titulo')}>
      <Checkbox
        label={t('editor.diferencas')}
        desc={t('editor.diferencasDesc')}
        checked={settings.data?.configDiffBeforeSave ?? true}
        disabled={pending}
        onChange={(checked) => {
          save({ configDiffBeforeSave: checked });
        }}
      />
    </SettingsPanel>
  );
}
