/**
 * Bloco recolhido **Enviado à IA: …** (SPEC T14 ✦ passo 2; protótipo `toolCall`; CA-T14-03):
 * aberto, mostra o que a IA pediu e, byte a byte, o que o Warden mandou de volta para ela (a
 * parte `functionResponse` exatamente como saiu na requisição). Consulta não permitida nesta
 * conversa aparece como tal, com o que foi respondido à IA no lugar.
 */
import { CircleAlert, Send } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { ToolStep } from '../../../lib/ipc/bindings';
import { cn } from '../../../lib/cn';
import { ScrollCode } from './ScrollCode';
import { sentSize, stepLabel } from './text';

export function StepBlock({ step }: { step: ToolStep }) {
  const { t } = useTranslation('ia');
  const failed = !step.ok;
  return (
    <details className={cn('toolcall', failed && 'toolcall--error')}>
      <summary>
        <Icon icon={failed ? CircleAlert : Send} />
        <span className="toolcall__title">
          {t('enviado.titulo', { oQue: stepLabel(step) })}
          {step.denied ? ` · ${t('enviado.negada')}` : failed ? ` · ${t('enviado.falhou')}` : ''}
        </span>
        <span className="toolcall__size">{sentSize(step.sent)}</span>
      </summary>
      <div className="toolcall__body">
        <div className="t-caps t-3">{t('enviado.pediu')}</div>
        <ScrollCode label={t('enviado.pediuRotulo')}>{`${step.name} ${step.args}`}</ScrollCode>
        <div className="t-caps t-3">{t('enviado.exato')}</div>
        <ScrollCode label={t('enviado.exatoRotulo')}>{step.sent}</ScrollCode>
      </div>
    </details>
  );
}

/** O texto inicial (ou outro texto do Warden) como foi enviado. */
export function SentBlock({ title, text }: { title: string; text: string }) {
  const { t } = useTranslation('ia');
  return (
    <details className="toolcall">
      <summary>
        <Icon icon={Send} />
        <span className="toolcall__title">{t('enviado.titulo', { oQue: title })}</span>
        <span className="toolcall__size">{sentSize(text)}</span>
      </summary>
      <div className="toolcall__body">
        <div className="t-caps t-3">{t('enviado.exato')}</div>
        <ScrollCode label={t('enviado.exatoRotulo')}>{text}</ScrollCode>
      </div>
    </details>
  );
}
