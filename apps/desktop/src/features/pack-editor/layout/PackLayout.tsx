/**
 * Moldura do pack aberto (SPEC T05; ADR-0026): cabeçalho fixo, menu lateral com as 6 seções e
 * a seção atual (`<Outlet />`) com rolagem própria. Só existe dentro de um pack: fora dele
 * nenhum item de pack aparece (CA-T05-03).
 *
 * Menu recolhido: uma rota filha declara `staticData: { packMenu: 'compact' }` (a página de
 * descoberta da P1-09, ESTRUTURA N6) e o menu vira só ícones enquanto ela estiver aberta.
 */
import { Outlet, useMatches } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { MAIN_ID } from '../../../app/layout/focus';
import { PageHead } from '../../../app/layout/PageHead';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import type { PackId, PackRow } from '../../../lib/ipc/bindings';
import { usePack } from '../../packs/api';
import { PackHeader } from '../header/PackHeader';
import { usePackWatch } from '../watch';
import { SectionMenu } from './SectionMenu';
import '../editor.css';

declare module '@tanstack/react-router' {
  interface StaticDataRouteOption {
    /** Estado do menu lateral do pack nesta rota (`compact`: só ícones). */
    packMenu?: 'compact';
  }
}

/** Se alguma rota aberta pede o menu recolhido. */
export function useCompactMenu(): boolean {
  return useMatches({
    select: (matches) => matches.some((match) => match.staticData.packMenu === 'compact'),
  });
}

export function PackLayout({ packId }: { packId: PackId }) {
  const { t } = useTranslation('editor');
  const pack = usePack(packId);
  const compact = useCompactMenu();
  usePackWatch(packId);
  const back = { to: '/packs', label: t('cabecalho.voltar') } as const;

  if (pack.isPending) {
    return (
      <AppPage back={back}>
        <LoadingState label={t('cabecalho.carregando')} />
      </AppPage>
    );
  }
  if (pack.isError) {
    return (
      <AppPage back={back}>
        <ErrorPanel
          error={pack.error}
          onRetry={() => {
            void pack.refetch();
          }}
        />
      </AppPage>
    );
  }
  if (pack.data.status !== 'ready') {
    return <UnavailablePack pack={pack.data} />;
  }
  return (
    <>
      <PackHeader pack={pack.data} />
      <div className="packbody">
        <SectionMenu packId={packId} compact={compact} />
        <main className="app__main" id={MAIN_ID} tabIndex={-1}>
          <div className="content">
            <Outlet />
          </div>
        </main>
      </div>
    </>
  );
}

/** Pasta sumida ou `pack.toml` ilegível: o pack não abre, e a tela diz por quê e o caminho. */
function UnavailablePack({ pack }: { pack: PackRow }) {
  const { t } = useTranslation('editor');
  const missing = pack.status === 'folderMissing';
  return (
    <AppPage back={{ to: '/packs', label: t('cabecalho.voltar') }} where={pack.name}>
      <div className="stack">
        <PageHead title={pack.name} />
        <Alert kind="danger" title={missing ? t('cabecalho.pastaSumiu') : t('cabecalho.ilegivel')}>
          <p>{missing ? t('cabecalho.pastaSumiuTexto') : t('cabecalho.ilegivelTexto')}</p>
          <div className="alert__text path">{pack.path}</div>
          {!missing && pack.detail ? <pre className="errpanel__pre">{pack.detail}</pre> : null}
        </Alert>
      </div>
    </AppPage>
  );
}
