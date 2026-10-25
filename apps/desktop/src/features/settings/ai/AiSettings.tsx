/**
 * Chaves e contas › IA (SPEC T21 e T14 ✦; ADR-0055 e ADR-0060; D-18), o encaixe da D-04 que
 * agora cobre os três provedores:
 *
 * - **Provedor do Sculk** das conversas novas: Gemini (o padrão, com plano gratuito), OpenAI ou
 *   Claude. As conversas já começadas continuam no provedor delas.
 * - **Chave de cada provedor**, na mesma linha das outras chaves (`KeyRow`: Testar, Substituir,
 *   Remover e o passo a passo com a página oficial).
 * - **Modelos do provedor ativo** (só informativo): na OpenAI e no Claude, o modelo de "Procurar
 *   rápido" e o de "Procurar a fundo", vindos da lista de modelos da chave (nenhum nome no
 *   código), com a frase de que a escolha é feita a cada pergunta no Sensor Sculk (ADR-0064) e
 *   uma frase curta sobre a cobrança por uso; no Gemini, uma linha só: "Usa sempre o modelo mais
 *   forte do plano gratuito", com o modelo em uso e a troca automática para um mais leve quando o
 *   limite diário acaba (feita e avisada pelo backend).
 */
import { Sparkles } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { AiProvider } from '../../../lib/ipc/bindings';
import { toAppError } from '../../../lib/ipc/errors';
import { useAiModels, useAiSettings, useSetAiProvider } from '../../diagnostics/ai/api';
import { useSecretsStatus } from '../api';
import { Choice } from '../components/fields';
import { KeyRow } from '../components/KeysSection';
import {
  AI_PROVIDERS,
  aiErrorMessage,
  modelFor,
  providerName,
  secretOf,
  usesStrongest,
} from './providers';
import './ai-settings.css';

/** Nome do grupo de rádio. */
const PROVIDER_GROUP = 'ia-provedor';

export function AiSettings() {
  const { t } = useTranslation('ia');
  const titleId = useId();
  const settings = useAiSettings();
  const secrets = useSecretsStatus();
  const setProvider = useSetAiProvider();
  const active = settings.data?.provider;

  return (
    <section className="ai-settings" aria-labelledby={titleId}>
      <h3 className="group-title" id={titleId}>
        <Icon icon={Sparkles} className="icon--ai" /> {t('configuracoes.titulo')}
      </h3>
      <p className="t-sm t-2">{t('configuracoes.texto')}</p>
      <fieldset className="ai-settings__group" aria-busy={setProvider.isPending}>
        <legend className="field__label">{t('configuracoes.provedor')}</legend>
        <div className="choice-list ai-settings__providers">
          {AI_PROVIDERS.map((provider) => (
            <Choice
              key={provider}
              name={PROVIDER_GROUP}
              title={providerName(provider)}
              badge={
                provider === 'gemini' ? (
                  <span className="tag tag--ok ml-2">{t('provedores.sugerido')}</span>
                ) : undefined
              }
              desc={t(`provedores.${provider}.desc`)}
              checked={active === provider}
              disabled={settings.isPending || setProvider.isPending}
              onSelect={() => {
                setProvider.mutate(provider, {
                  onSuccess: () => {
                    showToast({
                      kind: 'ok',
                      title: t('configuracoes.trocado', { provedor: providerName(provider) }),
                    });
                  },
                });
              }}
            />
          ))}
        </div>
        {setProvider.isError ? (
          <Alert kind="danger" compact>
            {aiErrorMessage(toAppError(setProvider.error), active ?? 'gemini')}
          </Alert>
        ) : null}
      </fieldset>
      <div className="ai-settings__group">
        <div className="field__label">{t('configuracoes.chaves')}</div>
        <div>
          {AI_PROVIDERS.map((provider) => (
            <KeyRow
              key={provider}
              kind={secretOf(provider)}
              configured={secrets.data?.[secretOf(provider)] ?? false}
              errorText={(error) => aiErrorMessage(toAppError(error), provider)}
            />
          ))}
        </div>
      </div>
      {active ? (
        <ModelSetting
          key={active}
          provider={active}
          keyConfigured={secrets.data?.[secretOf(active)] ?? false}
        />
      ) : null}
    </section>
  );
}

/** Os modelos do provedor ativo (só informativo; o modo é escolhido a cada pergunta). */
function ModelSetting({
  provider,
  keyConfigured,
}: {
  provider: AiProvider;
  keyConfigured: boolean;
}) {
  const { t } = useTranslation('ia');
  const models = useAiModels(provider, keyConfigured);
  const footer = (
    <>
      {!keyConfigured ? (
        <p className="t-xs t-3">{t(`configuracoes.semChave.${provider}`)}</p>
      ) : null}
      {models.isError ? (
        <Alert kind="warn" compact>
          {aiErrorMessage(toAppError(models.error), provider)}
        </Alert>
      ) : null}
    </>
  );

  if (provider === 'gemini') {
    const model = modelFor(models.data, 'deep');
    // O mais leve da troca na cota diária (sem ele, ou igual ao modelo, não há troca).
    const lighter = model && usesStrongest(models.data, model) ? models.data?.lighter : null;
    return (
      <div className="ai-settings__group" data-testid="ia-modelo-gemini">
        <div className="field__label">
          {t('configuracoes.modeloTitulo', { provedor: providerName(provider) })}
        </div>
        <div className="choice ai-settings__line">
          <span className="grow">
            <span className="choice__title">{t('configuracoes.gemini.titulo')}</span>
            <span className="choice__desc block">
              {model ? `${t('configuracoes.modelo', { modelo: model })} ` : ''}
              {lighter
                ? t('configuracoes.gemini.descLeve', { modelo: lighter })
                : t('configuracoes.gemini.desc')}
            </span>
          </span>
        </div>
        {footer}
      </div>
    );
  }

  const quick = models.data?.economical;
  const deep = models.data?.careful;
  return (
    <div className="ai-settings__group" data-testid="ia-modelos">
      <div className="field__label">
        {t('configuracoes.modeloTitulo', { provedor: providerName(provider) })}
      </div>
      <p className="t-sm t-2">
        {quick && deep
          ? t('configuracoes.modos', { rapido: quick, fundo: deep })
          : t('configuracoes.modosSemNome')}
      </p>
      <p className="t-sm t-2">{t(`configuracoes.custo.${provider}`)}</p>
      {footer}
    </div>
  );
}
