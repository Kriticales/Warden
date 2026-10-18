/**
 * As entradas para uma conversa nova já com um travamento (SPEC T14 ✦ "Onde"; protótipo
 * `teste-travou`): **✦ Pedir ajuda à IA** no resultado de um teste que travou (bloco do
 * resultado, `features/test/result-blocks.ts`) e **✦ Conversar com a IA** em cada linha de
 * Travamentos. Os dois levam à seção da IA com a sessão escolhida, onde o consentimento abre
 * sozinho; nada é enviado antes de **✦ Começar conversa**.
 */
import { Link } from '@tanstack/react-router';
import { Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { PackId } from '../../../lib/ipc/bindings';
import type { ResultBlockProps } from '../../test/result-blocks';
import { useCrashDiagnosis } from '../api';

/** Bloco do resultado do teste que travou. */
export function AskAiBlock({ packId, session }: ResultBlockProps) {
  const { t } = useTranslation('ia');
  const crashed = session.outcome === 'crashed';
  const diagnosis = useCrashDiagnosis(packId, session.id, crashed);
  if (!crashed || diagnosis.isPending) {
    return null;
  }
  const noCause = !diagnosis.data?.foundCause;
  return (
    <Alert
      kind="ai"
      title={noCause ? t('pedir.semCausa') : t('pedir.comCausa')}
      actions={
        <Button icon={Sparkles} asChild>
          <Link to="/packs/$packId/ia" params={{ packId }} search={{ sessao: session.id }}>
            {t('pedir.botao')}
          </Link>
        </Button>
      }
    >
      {t('pedir.texto')}
    </Alert>
  );
}

/** "✦ Conversar com a IA" numa linha de Travamentos. */
export function CrashAiButton({
  packId,
  sessionId,
  quando,
}: {
  packId: PackId;
  sessionId: string;
  quando: string;
}) {
  const { t } = useTranslation('ia');
  return (
    <Button size="sm" variant="ghost" icon={Sparkles} asChild>
      <Link
        to="/packs/$packId/ia"
        params={{ packId }}
        search={{ sessao: sessionId }}
        aria-label={t('travamentos.conversarRotulo', { quando })}
      >
        {t('travamentos.conversar')}
      </Link>
    </Button>
  );
}
