/**
 * Geral (T21): pasta dos packs, nome do jogador do teste e "Rever boas-vindas" (T01).
 */
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, buttonVariants } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { isValidPlayerName, useSettings } from '../api';
import { PacksDirField } from './PacksDirField';
import { PlayerNameField } from './PlayerNameField';
import { SettingsPanel, useSaveSetting } from './SettingsPanel';

export function GeneralSection() {
  const { t } = useTranslation('configuracoes');
  return (
    <SettingsPanel title={t('geral.titulo')}>
      <PacksDirField />
      <PlayerNameSetting />
      <div>
        <Link to="/boas-vindas" className={buttonVariants({ variant: 'link' })}>
          {t('geral.revisarBoasVindas')}
        </Link>
      </div>
    </SettingsPanel>
  );
}

function PlayerNameSetting() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const saved = settings.data?.playerName ?? '';
  const [draft, setDraft] = useState<string | null>(null);
  const { save, pending } = useSaveSetting();
  const value = draft ?? saved;
  const dirty = draft !== null && draft !== saved;
  const valid = isValidPlayerName(value);

  const submit = () => {
    if (!dirty || !valid || pending) {
      return;
    }
    save({ playerName: value }, () => {
      setDraft(null);
      showToast({ kind: 'ok', title: t('jogador.salvo') });
    });
  };

  return (
    <div className="stack-2">
      <PlayerNameField
        value={value}
        onChange={setDraft}
        hint={t('jogador.dica')}
        showError={draft !== null}
        onEnter={submit}
      />
      {dirty ? (
        <div>
          <Button size="sm" variant="primary" disabled={!valid} loading={pending} onClick={submit}>
            {t('jogador.salvar')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
