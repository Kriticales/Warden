/**
 * Configurações (T21): página única, rolável, sem submenu, com as seções de `sections.ts` na
 * ordem da SPEC e "Sobre o Warden" (T23) por último. Abre pela barra do app e volta para Meus
 * packs.
 */
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { useSettings } from '../api';
import { settingsSections } from '../sections';
import '../settings.css';

export function SettingsPage() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();

  let body;
  if (settings.isPending) {
    body = <LoadingState label={t('carregando')} />;
  } else if (settings.isError) {
    body = (
      <ErrorPanel
        error={settings.error}
        onRetry={() => {
          void settings.refetch();
        }}
      />
    );
  } else {
    body = settingsSections.map(({ id, Component }) => (
      <div key={id} id={id} className="settings__section">
        <Component />
      </div>
    ));
  }

  return (
    <AppPage back={{ to: '/', label: t('voltar') }} where={t('titulo')} hideLinks>
      <div className="settings">
        <PageHead title={t('titulo')} sub={t('subtitulo')} />
        {body}
      </div>
    </AppPage>
  );
}
