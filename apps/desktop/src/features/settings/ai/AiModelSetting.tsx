/**
 * O modelo da IA em Configurações → Chaves e contas (SPEC T14 ✦ "Na seção"; ADR-0055): **Mais
 * econômico** (padrão: o Flash-Lite estável mais novo) ou **Análise mais cuidadosa** (o Flash
 * estável mais novo, com o aviso de poucas conversas por dia no plano gratuito). Os nomes vêm do
 * `models.list` da chave; nenhum fica no código.
 */
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import type { ModelTier } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useAiModels, useAiSettings, useSetAiTier } from '../../diagnostics/ai/api';
import { useSecretsStatus } from '../api';
import { Choice } from '../components/fields';
import './ai-settings.css';

const TIERS: readonly ModelTier[] = ['economical', 'careful'];
/** Nome do grupo de rádio. */
const TIER_GROUP = 'ia-modelo';

export function AiModelSetting() {
  const { t } = useTranslation('ia');
  const secrets = useSecretsStatus();
  const keyConfigured = secrets.data?.gemini ?? false;
  const settings = useAiSettings();
  const models = useAiModels(keyConfigured);
  const setTier = useSetAiTier();
  const tier = settings.data?.tier ?? 'economical';
  const modelOf = (value: ModelTier) =>
    value === 'careful' ? models.data?.careful : models.data?.economical;
  return (
    <fieldset className="ai-settings">
      <legend className="field__label">{t('configuracoes.titulo')}</legend>
      <div className="choice-list">
        {TIERS.map((value) => {
          const model = modelOf(value);
          return (
            <Choice
              key={value}
              name={TIER_GROUP}
              title={t(`configuracoes.${value}.titulo`)}
              desc={
                <>
                  {t(`configuracoes.${value}.desc`)}
                  {model ? ` ${t('configuracoes.modelo', { modelo: model })}` : ''}
                </>
              }
              checked={tier === value}
              disabled={settings.isPending || setTier.isPending}
              onSelect={() => {
                setTier.mutate(value);
              }}
            />
          );
        })}
      </div>
      {!keyConfigured ? <p className="t-xs t-3">{t('configuracoes.semChave')}</p> : null}
      {models.isError ? (
        <Alert kind="warn" compact>
          {appErrorMessage(toAppError(models.error))}
        </Alert>
      ) : null}
      {setTier.isError ? (
        <Alert kind="danger" compact>
          {appErrorMessage(toAppError(setTier.error))}
        </Alert>
      ) : null}
    </fieldset>
  );
}
