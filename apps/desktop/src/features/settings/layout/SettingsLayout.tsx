/**
 * Configurações (T21; esboço aprovado `design/bug-01/configuracoes.html`, versão 2 · menu
 * lateral): a barra do app com "← Meus packs", o menu das 8 páginas à esquerda e a página
 * aberta (`/configuracoes/<página>`) à direita, na largura toda da janela.
 */
import { Outlet } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { MAIN_ID } from '../../../app/layout/focus';
import { PageTitle } from '../../../app/layout/PageHead';
import { TopBar } from '../../../app/layout/TopBar';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { useSettings } from '../api';
import { SettingsNav } from './SettingsNav';
import '../settings.css';

export function SettingsLayout() {
  const { t } = useTranslation('configuracoes');
  const settings = useSettings();

  // Lendo ou com erro, a página ainda não tem título: o da tela é "Configurações" (para o leitor
  // de tela e para o foco ao chegar).
  const pending = <PageTitle className="sr-only">{t('titulo')}</PageTitle>;
  let body;
  if (settings.isPending) {
    body = (
      <>
        {pending}
        <LoadingState label={t('carregando')} />
      </>
    );
  } else if (settings.isError) {
    body = (
      <div className="spage__pad">
        {pending}
        <ErrorPanel
          title={t('erro')}
          error={settings.error}
          onRetry={() => {
            void settings.refetch();
          }}
        />
      </div>
    );
  } else {
    body = <Outlet />;
  }

  return (
    <>
      <TopBar back={{ to: '/', label: t('voltar') }} where={t('titulo')} hideLinks />
      <div className="settings-body">
        <SettingsNav />
        <main className="settings-main" id={MAIN_ID} tabIndex={-1}>
          {body}
        </main>
      </div>
    </>
  );
}
