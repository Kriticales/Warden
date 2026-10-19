/**
 * Consentimento, uma vez por conversa (SPEC T14 ✦ passo 1; ADR-0030; protótipo `ia-consent`):
 * o que a IA poderá consultar (configs desmarcável; a busca no GitHub só quando existir, D-14),
 * o **texto inicial exato** já redigido, rolável, o tamanho, o provedor e o modelo (D-18:
 * Gemini, OpenAI ou Claude) e o custo. O custo de uma conversa típica e o aviso do plano
 * gratuito aparecem só no Gemini; na OpenAI e no Claude, a frase de que o uso é cobrado na chave,
 * sem números (CA-T14-24). **Cancelar** não manda nada (CA-T14-05): a prévia não levou nada do
 * pack para fora; só **✦ Começar conversa** grava e começa.
 */
import { Check, Sparkles } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import type { AiPreview, PackId } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { toAppError } from '../../../lib/ipc/errors';
import { aiErrorMessage, providerFullName } from '../../settings/ai/providers';
import { Checkbox } from '../../settings/components/fields';
import { useStartConversation } from './api';
import { ScrollCode } from './ScrollCode';
import { tokensLabel } from './text';

export interface ConsentDialogProps {
  packId: PackId;
  preview: AiPreview;
  /** Quem recebe o foco ao fechar. */
  returnFocusTo: () => HTMLElement | null;
  onClose: () => void;
  onStarted: (conversationId: string) => void;
}

export function ConsentDialog({
  packId,
  preview,
  returnFocusTo,
  onClose,
  onStarted,
}: ConsentDialogProps) {
  const { t } = useTranslation('ia');
  const start = useStartConversation(packId);
  const [configs, setConfigs] = useState(true);
  const [github, setGithub] = useState(true);
  const tier = preview.tier ?? 'economical';
  const provider = preview.provider;
  const gemini = provider === 'gemini';

  const fixed = (label: string, desc: string) => (
    <li className="row row--top ai-consent__item">
      <Icon icon={Check} className="t-ok" />
      <span>
        <b>{label}</b>
        <span className="check__desc block">{desc}</span>
      </span>
    </li>
  );

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={t('consentimento.titulo')}
        description={t('consentimento.sub')}
        returnFocusTo={returnFocusTo}
        footer={
          <>
            <DialogClose asChild>
              <Button variant="ghost">{t('consentimento.cancelar')}</Button>
            </DialogClose>
            <Button
              variant="primary"
              icon={Sparkles}
              loading={start.isPending}
              onClick={() => {
                start.mutate(
                  {
                    previewId: preview.previewId,
                    permissions: { configs, github: preview.githubAvailable && github },
                  },
                  { onSuccess: onStarted },
                );
              }}
            >
              {t('consentimento.comecar')}
            </Button>
          </>
        }
      >
        <div>
          <div className="field__label ai-consent__label">{t('consentimento.podeConsultar')}</div>
          <ul className="stack-2">
            {fixed(t('consentimento.pack'), t('consentimento.packDesc'))}
            {fixed(t('consentimento.achados'), t('consentimento.achadosDesc'))}
            {fixed(t('consentimento.logs'), t('consentimento.logsDesc'))}
            <li>
              <Checkbox
                label={<b>{t('consentimento.configs')}</b>}
                desc={t('consentimento.configsDesc')}
                checked={configs}
                onChange={setConfigs}
              />
            </li>
            {preview.githubAvailable ? (
              <li>
                <Checkbox
                  label={<b>{t('consentimento.github')}</b>}
                  desc={t('consentimento.githubDesc')}
                  checked={github}
                  onChange={setGithub}
                />
              </li>
            ) : null}
          </ul>
        </div>
        <div>
          <div className="field__label ai-consent__label">{t('consentimento.textoInicial')}</div>
          <ScrollCode className="ai-consent__text" label={t('consentimento.textoRotulo')}>
            {preview.text}
          </ScrollCode>
        </div>
        <dl className="kv">
          <dt>{t('consentimento.para')}</dt>
          <dd>
            {t('consentimento.paraValor', {
              provedor: providerFullName(provider),
              modelo: preview.model,
            })}
          </dd>
          <dt>{t('consentimento.tamanho')}</dt>
          <dd>
            {preview.estimatedTokens === null
              ? t('consentimento.tamanhoSoBytes', { bytes: formatBytes(preview.bytes) })
              : t('consentimento.tamanhoValor', {
                  bytes: formatBytes(preview.bytes),
                  tokens: tokensLabel(preview.estimatedTokens),
                })}
          </dd>
          <dt>{t('consentimento.custo')}</dt>
          <dd>
            {provider === 'gemini'
              ? t(`consentimento.custoValor.${tier}`)
              : t(`consentimento.custoPago.${provider}`)}
          </dd>
        </dl>
        {gemini ? <p className="t-sm t-2">{t('consentimento.plano')}</p> : null}
        {gemini && tier === 'careful' ? (
          <Alert kind="warn" compact>
            {t('consentimento.poucasPorDia')}
          </Alert>
        ) : null}
        {start.isError ? (
          <Alert kind="danger" compact title={t('consentimento.erro')}>
            {aiErrorMessage(toAppError(start.error), provider)}
          </Alert>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
