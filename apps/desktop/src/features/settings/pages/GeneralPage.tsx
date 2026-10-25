/**
 * Configurações → Geral (T21): pasta dos packs, nome do jogador nos testes e "Rever
 * boas-vindas" (T01).
 */
import { Link } from '@tanstack/react-router';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, buttonVariants } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { cn } from '../../../lib/cn';
import { isValidPlayerName, useSettings } from '../api';
import { PacksDirField } from '../components/PacksDirField';
import { SettingRow, SettingsFrame, useRowLabel } from '../layout/SettingsFrame';
import { useSaveSetting } from '../layout/save';

export function GeneralPage() {
  const { t } = useTranslation('configuracoes');
  return (
    <SettingsFrame page="geral">
      <div className="sform">
        <SettingRow title={t('pasta.rotulo')} desc={t('pasta.dica')}>
          <PacksDirField inRow />
        </SettingRow>
        <SettingRow title={t('jogador.rotulo')} desc={t('jogador.dica')}>
          <PlayerName />
        </SettingRow>
        <SettingRow title={t('geral.boasVindas')} desc={t('geral.boasVindasDesc')}>
          <Link to="/boas-vindas" className={buttonVariants()}>
            {t('geral.revisarBoasVindas')}
          </Link>
        </SettingRow>
      </div>
    </SettingsFrame>
  );
}

function PlayerName() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();
  const label = useRowLabel();
  const saved = settings.data?.playerName ?? '';
  const [draft, setDraft] = useState<string | null>(null);
  const { save, pending } = useSaveSetting();
  const value = draft ?? saved;
  const dirty = draft !== null && draft !== saved;
  const valid = isValidPlayerName(value);
  const hintId = useId();

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
      <div className="field__row">
        <input
          className="input"
          value={value}
          maxLength={32}
          spellCheck={false}
          aria-invalid={valid ? undefined : true}
          aria-labelledby={label['aria-labelledby']}
          aria-describedby={hintId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              submit();
            }
          }}
        />
        {dirty ? (
          <Button variant="primary" disabled={!valid} loading={pending} onClick={submit}>
            {t('jogador.salvar')}
          </Button>
        ) : null}
      </div>
      <p
        className={cn(valid ? 'field__hint' : 'field__error')}
        id={hintId}
        role={valid ? undefined : 'alert'}
      >
        {valid ? t('jogador.regra') : t('jogador.invalido')}
      </p>
    </div>
  );
}
