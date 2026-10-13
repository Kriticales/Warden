/**
 * "Mods opcionais ligados neste perfil" dos Ajustes do teste (SPEC T11 "Perfis do teste";
 * ROADMAP L-08), para os perfis fora do Padrão. O Padrão continua com as escolhas da instância
 * (`OptionalChoicesSection`, que grava na hora). Aqui a escolha fica no rascunho do perfil e é
 * gravada com "Salvar ajustes"; sem escolha no perfil, vale o que está ligado no Padrão.
 */
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import type { PackId } from '../../../lib/ipc/bindings';
import { Switch } from '../../settings/components/fields';
import { useOptionalChoices } from '../item-options/api';

export interface ProfileOptionalsProps {
  packId: PackId;
  /** As escolhas do perfil (`null` = as do Padrão). */
  value: Partial<Record<string, boolean>> | null;
  onChange: (value: Record<string, boolean>) => void;
}

export function ProfileOptionals({ packId, value, onChange }: ProfileOptionalsProps) {
  const { t } = useTranslation('perfis');
  const { t: to } = useTranslation('opcionais');
  const choices = useOptionalChoices(packId);
  if (choices.isError) {
    return (
      <ErrorPanel
        compact
        error={choices.error}
        onRetry={() => {
          void choices.refetch();
        }}
      />
    );
  }
  if (!choices.isSuccess) {
    return <LoadingState inline label={to('teste.carregando')} />;
  }
  // Sem escolha no perfil, vale o que está ligado na instância (o Padrão).
  const rows = choices.data.map((row) => ({ ...row, enabled: value?.[row.path] ?? row.enabled }));
  return (
    <section className="stack-2" aria-labelledby={`perfil-opcionais-${packId}`}>
      <div className="field__label" id={`perfil-opcionais-${packId}`}>
        {t('opcionais.titulo')}
      </div>
      <p className="t-sm t-2">{t('opcionais.texto')}</p>
      {rows.length === 0 ? (
        <p className="t-sm t-3">{to('teste.nenhum')}</p>
      ) : (
        <ul className="stack-2" aria-label={to('teste.lista')}>
          {rows.map((row) => (
            <li key={row.path} className="stack-1">
              <Switch
                label={row.name}
                checked={row.enabled}
                onText={to('ligado')}
                offText={to('desligado')}
                onChange={(enabled) => {
                  const next: Record<string, boolean> = Object.fromEntries(
                    rows.map((other) => [other.path, other.enabled]),
                  );
                  next[row.path] = enabled;
                  onChange(next);
                }}
              />
              <div className="field__hint">
                {row.description ? `${row.description}. ` : ''}
                {to('teste.padrao', {
                  estado: row.default ? to('teste.ligado') : to('teste.desligado'),
                })}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
