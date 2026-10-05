import { createFileRoute } from '@tanstack/react-router';
import { Trans, useTranslation } from 'react-i18next';

import { AppPage } from '../app/layout/AppPage';
import { PageTitle } from '../app/layout/PageHead';
import { AboutWarden } from '../features/about/components/AboutWarden';
import { redirectOnFirstRun } from '../features/onboarding/firstRun';

export const Route = createFileRoute('/')({
  // Primeira execução (T01, P1-13): sem `settings.json`, o app abre nas boas-vindas.
  beforeLoad: ({ context }) => redirectOnFirstRun(context.queryClient),
  component: HomePage,
});

/**
 * Página inicial provisória (F0-06): a moldura do app com as boas-vindas e "Sobre o Warden".
 * A lista "Meus packs" (T02) substitui este conteúdo quando existir.
 */
function HomePage() {
  const { t } = useTranslation();
  return (
    <AppPage narrow>
      <div className="stack">
        <div>
          <PageTitle className="t-display-2xl">
            <Trans i18nKey="app.boasVindas" components={{ hl: <span className="t-hl" /> }} />
          </PageTitle>
          <p className="t-2 measure mt-3">{t('app.descricao')}</p>
        </div>
        <AboutWarden />
      </div>
    </AppPage>
  );
}
