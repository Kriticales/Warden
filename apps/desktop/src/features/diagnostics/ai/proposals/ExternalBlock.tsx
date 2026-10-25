/**
 * **Enviado ao GitHub: …** dentro da consulta do Sculk (SPEC T14 passo 2; CA-T14-03): byte a
 * byte o pedido que saiu (`GET …`, só o repositório e as palavras do erro; o token nunca
 * aparece). Quando a resposta veio do cache de uma busca igual, diz que nada foi enviado de novo.
 * O que voltou para o Sculk fica logo abaixo, em "O que foi enviado".
 */
import { useTranslation } from 'react-i18next';

import type { ExternalRequest } from '../../../../lib/ipc/bindings';
import { RawBlock } from '../Thread';
import { sentSize } from '../text';

export function ExternalBlock({ request }: { request: ExternalRequest }) {
  const { t } = useTranslation('iaPropostas');
  return (
    <div data-testid="enviado-github">
      <div className="tc__label">
        {t('github.titulo', { repositorio: request.repository, termos: request.terms })} ·{' '}
        <span className="mono">{sentSize(request.request)}</span>
      </div>
      {request.cached ? <p className="t-sm t-2">{t('github.cache')}</p> : null}
      <RawBlock
        text={request.request}
        head={t('github.exato')}
        label={t('github.exatoRotulo')}
        wrap
      />
    </div>
  );
}
