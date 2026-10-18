/**
 * Bloco recolhido **Enviado ao GitHub: …** (SPEC T14 ✦ passo 2; protótipo `toolCall` com
 * `kind: "github"`; CA-T14-03): aberto, mostra byte a byte o pedido que saiu (`GET …`, só o
 * repositório e as palavras do erro; o token nunca aparece). Quando a resposta veio do cache de
 * uma busca igual, diz que nada foi enviado de novo. O que voltou para a IA fica no bloco
 * "Enviado à IA" da mesma consulta, logo abaixo.
 */
import { Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../../components/ui/icon';
import type { ExternalRequest } from '../../../../lib/ipc/bindings';
import { ScrollCode } from '../ScrollCode';
import { sentSize } from '../text';

export function ExternalBlock({ request }: { request: ExternalRequest }) {
  const { t } = useTranslation('iaPropostas');
  return (
    <details className="toolcall" data-testid="enviado-github">
      <summary>
        <Icon icon={Globe} />
        <span className="toolcall__title">
          {t('github.titulo', { repositorio: request.repository, termos: request.terms })}
        </span>
        <span className="toolcall__size">{sentSize(request.request)}</span>
      </summary>
      <div className="toolcall__body">
        {request.cached ? <p className="t-sm t-2">{t('github.cache')}</p> : null}
        <div className="t-caps t-3">{t('github.exato')}</div>
        <ScrollCode label={t('github.exatoRotulo')}>{request.request}</ScrollCode>
      </div>
    </details>
  );
}
