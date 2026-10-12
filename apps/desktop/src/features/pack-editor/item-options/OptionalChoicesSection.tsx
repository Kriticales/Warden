/**
 * "Mods opcionais" dos Ajustes do teste neste computador (SPEC T11): quais opcionais ficam
 * ligados na instância de teste. Sem escolha, vale o "ligado por padrão" do pack. Dado desta
 * máquina (`state/optional-choices.json`): não entra no pack e não conta como alteração.
 * Cada interruptor grava na hora; o efeito vale a partir do próximo teste.
 */
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import type { OptionalChoice, PackId } from '../../../lib/ipc/bindings';
import { Switch } from '../../settings/components/fields';
import { useOptionalChoices, useSetOptionalChoices } from './api';

export function OptionalChoicesSection({ packId }: { packId: PackId }) {
  const { t } = useTranslation('opcionais');
  const choices = useOptionalChoices(packId);
  const save = useSetOptionalChoices(packId);
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
    return <LoadingState inline label={t('teste.carregando')} />;
  }
  const rows = choices.data;
  const toggle = (row: OptionalChoice, enabled: boolean) => {
    const next: Record<string, boolean> = Object.fromEntries(
      rows.map((other) => [other.path, other.enabled]),
    );
    next[row.path] = enabled;
    save.mutate(next);
  };
  return (
    <section className="stack-2" aria-labelledby={`opcionais-${packId}`}>
      <div className="field__label" id={`opcionais-${packId}`}>
        {t('teste.titulo')}
      </div>
      <p className="t-sm t-2">{t('teste.texto')}</p>
      {rows.length === 0 ? (
        <p className="t-sm t-3">{t('teste.nenhum')}</p>
      ) : (
        <ul className="stack-2" aria-label={t('teste.lista')}>
          {rows.map((row) => (
            <li key={row.path} className="stack-1">
              <Switch
                label={row.name}
                checked={row.enabled}
                disabled={save.isPending}
                onText={t('ligado')}
                offText={t('desligado')}
                onChange={(enabled) => {
                  toggle(row, enabled);
                }}
              />
              <div className="field__hint">
                {row.description ? `${row.description}. ` : ''}
                {t('teste.padrao', {
                  estado: row.default ? t('teste.ligado') : t('teste.desligado'),
                })}
              </div>
            </li>
          ))}
        </ul>
      )}
      {save.isError ? <ErrorPanel compact error={save.error} /> : null}
      <p className="t-xs t-3">{t('teste.aplicaNoProximo')}</p>
    </section>
  );
}
