/**
 * Consentimento da conversa sem pack, uma vez por conversa (ROADMAP D-19; SPEC T14 item 8;
 * ADR-0030): as mesmas regras do consentimento com pack, só com o que existe sem pack. A IA pode
 * consultar o log escolhido (sempre sem dados pessoais) e, com a caixa marcada, procurar issues
 * parecidas no GitHub. Mostra o **texto inicial exato**, já redigido, o tamanho, o provedor e o
 * modelo e o custo. **Cancelar** não manda nada à IA: a prévia só baixou o texto do link.
 */
import { Check, Sparkles } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import type { AiPreview } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { Checkbox } from '../../settings/components/fields';
import { ScrollCode } from '../ai/ScrollCode';
import { tokensLabel } from '../ai/text';
import { useStartStandalone } from './api';

export interface StandaloneConsentProps {
  preview: AiPreview;
  /** Quem recebe o foco ao fechar. */
  returnFocusTo: () => HTMLElement | null;
  onClose: () => void;
  onStarted: (conversationId: string) => void;
}

export function StandaloneConsent({
  preview,
  returnFocusTo,
  onClose,
  onStarted,
}: StandaloneConsentProps) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const start = useStartStandalone();
  const [github, setGithub] = useState(true);
  const tier = preview.tier ?? 'economical';

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={ti('consentimento.titulo')}
        description={t('consentimento.sub')}
        returnFocusTo={returnFocusTo}
        footer={
          <>
            <DialogClose asChild>
              <Button variant="ghost">{ti('consentimento.cancelar')}</Button>
            </DialogClose>
            <Button
              variant="primary"
              icon={Sparkles}
              loading={start.isPending}
              onClick={() => {
                start.mutate(
                  {
                    previewId: preview.previewId,
                    permissions: { configs: false, github: preview.githubAvailable && github },
                  },
                  { onSuccess: onStarted },
                );
              }}
            >
              {ti('consentimento.comecar')}
            </Button>
          </>
        }
      >
        <div>
          <div className="field__label ai-consent__label">{ti('consentimento.podeConsultar')}</div>
          <ul className="stack-2">
            <li className="row row--top ai-consent__item">
              <Icon icon={Check} className="t-ok" />
              <span>
                <b>{t('consentimento.log')}</b>
                <span className="check__desc block">{ti('consentimento.logsDesc')}</span>
              </span>
            </li>
            {preview.githubAvailable ? (
              <li>
                <Checkbox
                  label={<b>{ti('consentimento.github')}</b>}
                  desc={ti('consentimento.githubDesc')}
                  checked={github}
                  onChange={setGithub}
                />
              </li>
            ) : null}
          </ul>
          <p className="t-sm t-3">{t('consentimento.semPack')}</p>
        </div>
        <div>
          <div className="field__label ai-consent__label">{ti('consentimento.textoInicial')}</div>
          <ScrollCode className="ai-consent__text" label={ti('consentimento.textoRotulo')}>
            {preview.text}
          </ScrollCode>
        </div>
        <dl className="kv">
          <dt>{ti('consentimento.para')}</dt>
          <dd>{ti('consentimento.paraValor', { modelo: preview.model })}</dd>
          <dt>{ti('consentimento.tamanho')}</dt>
          <dd>
            {preview.estimatedTokens === null
              ? ti('consentimento.tamanhoSoBytes', { bytes: formatBytes(preview.bytes) })
              : ti('consentimento.tamanhoValor', {
                  bytes: formatBytes(preview.bytes),
                  tokens: tokensLabel(preview.estimatedTokens),
                })}
          </dd>
          <dt>{ti('consentimento.custo')}</dt>
          <dd>{ti(`consentimento.custoValor.${tier}`)}</dd>
        </dl>
        <p className="t-sm t-2">{ti('consentimento.plano')}</p>
        {tier === 'careful' ? (
          <Alert kind="warn" compact>
            {ti('consentimento.poucasPorDia')}
          </Alert>
        ) : null}
        {start.isError ? (
          <Alert kind="danger" compact title={ti('consentimento.erro')}>
            {appErrorMessage(toAppError(start.error))}
          </Alert>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
