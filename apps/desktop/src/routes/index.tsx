import { useQuery } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { commands } from '../lib/ipc/bindings';
import { unwrap } from '../lib/ipc/result';

export const Route = createFileRoute('/')({
  component: HomePage,
});

/** Página inicial provisória (F0-01): nome, descrição e versão vinda de `app_info`. */
function HomePage() {
  const { t } = useTranslation();
  const info = useQuery({
    queryKey: ['app', 'info'],
    queryFn: async () => unwrap(await commands.appInfo()),
  });

  let version: string;
  if (info.isPending) {
    version = t('app.carregandoVersao');
  } else if (info.isError) {
    version = t('app.versaoIndisponivel');
  } else if (info.data.commit) {
    version = t('app.versaoComCommit', { version: info.data.version, commit: info.data.commit });
  } else {
    version = t('app.versao', { version: info.data.version });
  }

  return (
    <main className="home">
      <h1 className="home__title">{t('app.nome')}</h1>
      <p className="home__description">{t('app.descricao')}</p>
      <p className="home__version" role="status">
        {version}
      </p>
    </main>
  );
}
