/**
 * "Sobre o Warden" (T23): versão e commit, o aviso legal da Mojang/Microsoft (T01, CA-T01-04)
 * e o link de apoio ao Forge (pedido do próprio instalador do Forge; R2 §3.4).
 *
 * A P1-13 encaixa este componente como a última seção de Configurações (registro
 * `features/settings/sections.ts`); as licenças de terceiros entram pela A-02 (`licenses/`).
 */
import { ExternalLink } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import { openExternal } from '../../../lib/external';
import { useAppInfo } from '../hooks/useAppInfo';

/** Página de apoio indicada pelo instalador do Forge (R2 §3.4). */
export const FORGE_SUPPORT_URL = 'https://www.patreon.com/LexManos/';

export interface AboutWardenProps {
  /** Nível do título, para seguir a hierarquia da página. Padrão: 2. */
  headingLevel?: 2 | 3;
}

export function AboutWarden({ headingLevel = 2 }: AboutWardenProps) {
  const { t } = useTranslation('sobre');
  const titleId = useId();
  const info = useAppInfo();
  const Heading = headingLevel === 2 ? 'h2' : 'h3';

  let facts;
  if (info.isPending) {
    facts = <LoadingState inline label={t('carregando')} />;
  } else if (info.isError) {
    facts = (
      <ErrorPanel
        compact
        error={info.error}
        onRetry={() => {
          void info.refetch();
        }}
      />
    );
  } else {
    facts = (
      <dl className="kv">
        <dt>{t('versao')}</dt>
        <dd>{info.data.version}</dd>
        {info.data.commit ? (
          <>
            <dt>{t('commit')}</dt>
            <dd className="t-mono">{info.data.commit}</dd>
          </>
        ) : null}
        {info.data.debugBuild ? (
          <>
            <dt>{t('compilacao')}</dt>
            <dd>{t('compilacaoDesenvolvimento')}</dd>
          </>
        ) : null}
      </dl>
    );
  }

  return (
    <section className="panel" aria-labelledby={titleId}>
      <Heading className="panel__title" id={titleId}>
        {t('titulo')}
      </Heading>
      <div className="stack-3 mt-3">
        {facts}
        <p className="legal" data-testid="aviso-legal">
          {t('avisoLegal')}
        </p>
        <p className="t-sm t-2">{t('offline')}</p>
      </div>
      <div className="btn-row about__links">
        <Tooltip content={t('apoiarForgeDica')}>
          <Button
            variant="link"
            iconEnd={ExternalLink}
            onClick={() => {
              void openExternal(FORGE_SUPPORT_URL);
            }}
          >
            {t('apoiarForge')}
          </Button>
        </Tooltip>
      </div>
    </section>
  );
}
