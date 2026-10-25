/**
 * Consentimento da conversa sem pack, uma vez por conversa (ROADMAP D-19; SPEC T14 item 8;
 * ADR-0030): a mesma janela da conversa com pack (`ConsentDialog`), só com o que existe sem pack.
 * O Sculk pode consultar o log escolhido (sempre sem dados pessoais) e, com a caixa marcada,
 * procurar issues parecidas no GitHub. **Cancelar** não manda nada ao Sculk: a prévia só baixou o
 * texto do link.
 */
import { useTranslation } from 'react-i18next';

import type { AiPreview } from '../../../lib/ipc/bindings';
import { ConsentDialog } from '../ai/ConsentDialog';
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
  return (
    <ConsentDialog
      preview={preview}
      fixed={[{ label: t('consentimento.log'), desc: ti('consentimento.logsDesc') }]}
      showConfigs={false}
      note={t('consentimento.semPack')}
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
