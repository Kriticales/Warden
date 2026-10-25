/**
 * Consentimento, uma vez por conversa (SPEC T14 passo 1; ADR-0030; protótipo
 * `design/bug-01/sensor-sculk.html`): **O que o Sculk poderá consultar** (o que vale sempre, com
 * a marca "Sempre", e o que dá para desmarcar: configs e a busca no GitHub, D-14), o **texto
 * inicial exato** já redigido, rolável e com Copiar, para quem vai (provedor e modelo, D-18), o
 * tamanho e o custo. O custo de uma conversa típica e o aviso do plano gratuito aparecem só no
 * Gemini, pelo modelo de verdade (ADR-0060); na OpenAI e no Claude, a frase de que o uso é
 * cobrado na chave, sem números (CA-T14-24). **Cancelar** não manda nada (CA-T14-05): a prévia
 * não levou nada do pack para fora; só **Enviar e começar** grava e começa.
 *
 * A mesma janela serve à conversa sem pack ("Analisar um log", D-19), com a lista do que existe
 * sem pack.
 */
import { Check, Copy, Sparkles } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import type { AiPermissions, AiPreview, PackId } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { toAppError } from '../../../lib/ipc/errors';
import { aiErrorMessage, providerFullName, usesStrongest } from '../../settings/ai/providers';
import { Checkbox } from '../../settings/components/fields';
import { useAiModels, useStartConversation } from './api';
import { useCopy } from './Thread';
import { tokensLabel } from './text';
import './sculk.css';

/** O que vale sempre na conversa (marcado "Sempre"). */
export interface FixedPermission {
  label: string;
  desc: string;
}

export interface ConsentDialogProps {
  preview: AiPreview;
  /** O que o Sculk consulta sempre nesta conversa. */
  fixed: readonly FixedPermission[];
  /** A caixa "Configs do pack" (só com pack). */
  showConfigs: boolean;
  /** Uma frase depois da lista (sem pack: o que o Sculk não vê). */
  note?: string;
  starting: boolean;
  error: unknown;
  /** "Enviar e começar", com as permissões escolhidas. */
  onStart: (permissions: AiPermissions) => void;
  /** Quem recebe o foco ao fechar. */
  returnFocusTo: () => HTMLElement | null;
  onClose: () => void;
}

export function ConsentDialog({
  preview,
  fixed,
  showConfigs,
  note,
  starting,
  error,
  onStart,
  returnFocusTo,
  onClose,
}: ConsentDialogProps) {
  const { t } = useTranslation('ia');
  const ids = useId();
  const copy = useCopy();
  const [configs, setConfigs] = useState(true);
  const [github, setGithub] = useState(true);
  const provider = preview.provider;
  const gemini = provider === 'gemini';
  // A lista de modelos já está no cache (a tela a consulta antes de abrir o consentimento).
  const models = useAiModels(provider, gemini);
  const strongest = gemini && usesStrongest(models.data, preview.model);

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={
          <>
            <Icon icon={Sparkles} className="icon--ai" /> {t('consentimento.titulo')}
          </>
        }
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
              loading={starting}
              onClick={() => {
                onStart({
                  configs: showConfigs && configs,
                  github: preview.githubAvailable && github,
                });
              }}
            >
              {t('consentimento.comecar')}
            </Button>
          </>
        }
      >
        <div className="consent stack-5">
          <section aria-labelledby={`${ids}-pode`}>
            <h3 id={`${ids}-pode`}>{t('consentimento.podeConsultar')}</h3>
            <ul className="consent__perms">
              {fixed.map((item) => (
                <li key={item.label} className="perm">
                  <Icon icon={Check} />
                  <span className="perm__t">{item.label}</span>
                  <span className="perm__fixed">{t('consentimento.sempre')}</span>
                  <span className="perm__d">{item.desc}</span>
                </li>
              ))}
              {showConfigs ? (
                <li className="perm perm--check">
                  <Checkbox
                    label={<b>{t('consentimento.configs')}</b>}
                    desc={t('consentimento.configsDesc')}
                    checked={configs}
                    onChange={setConfigs}
                  />
                </li>
              ) : null}
              {preview.githubAvailable ? (
                <li className="perm perm--check">
                  <Checkbox
                    label={<b>{t('consentimento.github')}</b>}
                    desc={t('consentimento.githubDesc')}
                    checked={github}
                    onChange={setGithub}
                  />
                </li>
              ) : null}
            </ul>
            {note ? <p className="fhint">{note}</p> : null}
          </section>
          <section aria-labelledby={`${ids}-texto`}>
            <div className="rawhead">
              <h3 id={`${ids}-texto`}>{t('consentimento.textoInicial')}</h3>
              <Button
                size="sm"
                variant="ghost"
                icon={Copy}
                onClick={() => {
                  copy(preview.text);
                }}
              >
                {t('consentimento.copiar')}
              </Button>
            </div>
            <pre
              className="raw raw--short"
              // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
              tabIndex={0}
              aria-label={t('consentimento.textoRotulo')}
            >
              {preview.text}
            </pre>
          </section>
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
              <span className="sub">{t('consentimento.token')}</span>
            </dd>
            <dt>{t('consentimento.custo')}</dt>
            <dd>
              {gemini
                ? t(`consentimento.custoValor.${strongest ? 'forte' : 'leve'}`)
                : t(`consentimento.custoPago.${provider}`)}
            </dd>
          </dl>
          {gemini ? <p className="t-sm t-2">{t('consentimento.plano')}</p> : null}
          {strongest ? (
            <Alert kind="warn" compact>
              {t('consentimento.poucasPorDia')}
            </Alert>
          ) : null}
          <Alert kind="ai" compact>
            {t('consentimento.aviso')}
          </Alert>
          {error ? (
            <Alert kind="danger" compact title={t('consentimento.erro')}>
              {aiErrorMessage(toAppError(error), provider)}
            </Alert>
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** O consentimento da conversa com pack: o que o Sculk vê do pack e dos testes. */
export function PackConsentDialog({
  packId,
  preview,
  returnFocusTo,
  onClose,
  onStarted,
}: {
  packId: PackId;
  preview: AiPreview;
  returnFocusTo: () => HTMLElement | null;
  onClose: () => void;
  onStarted: (conversationId: string) => void;
}) {
  const { t } = useTranslation('ia');
  const start = useStartConversation(packId);
  return (
    <ConsentDialog
      preview={preview}
      fixed={[
        { label: t('consentimento.pack'), desc: t('consentimento.packDesc') },
        { label: t('consentimento.achados'), desc: t('consentimento.achadosDesc') },
        { label: t('consentimento.logs'), desc: t('consentimento.logsDesc') },
        { label: t('consentimento.desempenho'), desc: t('consentimento.desempenhoDesc') },
      ]}
      showConfigs
      starting={start.isPending}
      error={start.isError ? start.error : null}
      onStart={(permissions) => {
        start.mutate({ previewId: preview.previewId, permissions }, { onSuccess: onStarted });
      }}
      returnFocusTo={returnFocusTo}
      onClose={onClose}
    />
  );
}
