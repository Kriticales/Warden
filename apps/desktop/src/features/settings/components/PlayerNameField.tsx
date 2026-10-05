/**
 * Nome do jogador no perfil offline do teste (T01 passo 2; T21 → Geral). A regra é a do
 * Minecraft, a mesma que o Rust confere no `settings_update`: 3 a 16 caracteres, letras sem
 * acento, números e `_` (CA-T01-02: `Zé` é recusado, `Ze_123` é aceito).
 */
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { isValidPlayerName } from '../api';
import { TextField } from './fields';

export interface PlayerNameFieldProps {
  value: string;
  onChange: (value: string) => void;
  label?: ReactNode;
  hint?: ReactNode;
  /** Mostra o erro mesmo sem o usuário ter digitado (ao tentar avançar). */
  showError?: boolean;
  onEnter?: () => void;
  id?: string;
}

export function PlayerNameField({
  value,
  onChange,
  label,
  hint,
  showError = true,
  onEnter,
  id,
}: PlayerNameFieldProps) {
  const { t } = useTranslation('configuracoes');
  const invalid = showError && !isValidPlayerName(value);
  return (
    <TextField
      id={id}
      label={label ?? t('jogador.rotulo')}
      value={value}
      maxLength={32}
      error={invalid ? t('jogador.invalido') : null}
      hint={invalid ? null : (hint ?? t('jogador.regra'))}
      onChange={(event) => {
        onChange(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          onEnter?.();
        }
      }}
    />
  );
}
