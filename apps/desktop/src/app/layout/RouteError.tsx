/**
 * Erro inesperado numa rota (QUALITY §3.9): o *error boundary* de cada rota mostra o
 * `ErrorPanel` com "Tentar de novo", que recarrega os dados da rota e desmonta o erro.
 *
 * Numa página do nível do app, o erro ocupa a página inteira (com a barra do app, para dar
 * para sair dali); dentro de um layout (o do pack, por exemplo), ocupa só o conteúdo.
 */
import { useMatch, useMatches, useRouter, type ErrorComponentProps } from '@tanstack/react-router';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { log } from '../../lib/log';
import { AppPage } from './AppPage';
import { PageTitle } from './PageHead';

export function RouteError({ error, reset }: ErrorComponentProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const match = useMatch({ strict: false });
  const matches = useMatches();
  // Rota filha direta da raiz (o primeiro match é a raiz) = página do nível do app.
  const appLevel = matches.findIndex((item) => item.id === match.id) <= 1;

  useEffect(() => {
    log.error(`erro na rota ${match.routeId}`, error);
  }, [error, match.routeId]);

  const panel = (
    <div className="stack">
      <PageTitle className="pagehead__title">{t('erro.telaTitulo')}</PageTitle>
      <ErrorPanel
        error={error}
        onRetry={() => {
          void router.invalidate().finally(reset);
        }}
      />
    </div>
  );
  return appLevel ? <AppPage>{panel}</AppPage> : panel;
}
