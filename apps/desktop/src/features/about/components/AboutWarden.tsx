/**
 * Configurações → Sobre o Warden (T23; esboço `design/bug-01/configuracoes.html`): a versão, o
 * commit e o commit do packwiz embutido; o aviso legal da Mojang/Microsoft (T01, CA-T01-04); os
 * créditos; as licenças de terceiros (A-02, `licenses/`) e o link de apoio ao Forge (pedido do
 * próprio instalador do Forge; R2 §3.4).
 */
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import { openExternal } from '../../../lib/external';
import { SettingRow, SettingsFrame } from '../../settings/layout/SettingsFrame';
import { useAppInfo } from '../hooks/useAppInfo';
import { ThirdPartyLicenses } from '../licenses/ThirdPartyLicenses';

/** Página de apoio indicada pelo instalador do Forge (R2 §3.4). */
export const FORGE_SUPPORT_URL = 'https://www.patreon.com/LexManos/';

/** O commit do packwiz em 7 caracteres, como o Git mostra. */
const SHORT_COMMIT = 7;

const CREDITS = ['packwiz', 'portablemc', 'forge'] as const;

export function AboutWarden() {
  const { t } = useTranslation('sobre');
  return (
    <SettingsFrame page="sobre">
      <div className="sform">
        <SettingRow title={t('versao')}>
          <VersionFacts />
        </SettingRow>
        <SettingRow title={t('avisoLegalTitulo')} wide>
          <p className="legal" data-testid="aviso-legal">
            {t('avisoLegal')}
          </p>
          <p className="t-sm t-2 mt-2">{t('offline')}</p>
        </SettingRow>
        <SettingRow title={t('creditosTitulo')} wide>
          <ul className="credits">
            {CREDITS.map((credit) => (
              <li key={credit}>
                <b>{t(`creditos.${credit}.nome`)}</b>: {t(`creditos.${credit}.texto`)}
              </li>
            ))}
          </ul>
        </SettingRow>
        <SettingRow title={t('licencasTitulo')} desc={t('licencasDesc')}>
          <div className="btn-row">
            <ThirdPartyLicenses />
            <Tooltip content={t('apoiarForgeDica')}>
              <Button
                variant="ghost"
                icon={ExternalLink}
                onClick={() => {
                  void openExternal(FORGE_SUPPORT_URL);
                }}
              >
                {t('apoiarForge')}
              </Button>
            </Tooltip>
          </div>
        </SettingRow>
      </div>
    </SettingsFrame>
  );
}

function VersionFacts() {
  const { t } = useTranslation('sobre');
  const info = useAppInfo();
  if (info.isPending) {
    return <LoadingState inline label={t('carregando')} />;
  }
  if (info.isError) {
    return (
      <ErrorPanel
        compact
        error={info.error}
        onRetry={() => {
          void info.refetch();
        }}
      />
    );
  }
  return (
    <dl className="kv about-kv">
      <dt>{t('warden')}</dt>
      <dd className="t-mono">{info.data.version}</dd>
      {info.data.commit ? (
        <>
          <dt>{t('commit')}</dt>
          <dd className="t-mono">{info.data.commit}</dd>
        </>
      ) : null}
      <dt>{t('packwiz')}</dt>
      <dd className="t-mono" title={info.data.packwizCommit}>
        {info.data.packwizCommit.slice(0, SHORT_COMMIT)}
      </dd>
      {info.data.debugBuild ? (
        <>
          <dt>{t('compilacao')}</dt>
          <dd>{t('compilacaoDesenvolvimento')}</dd>
        </>
      ) : null}
    </dl>
  );
}
