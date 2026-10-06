/**
 * Página do pack, provisória (P1-07): para onde Abrir, Criar pack e Abrir pack levam até o
 * editor do pack existir (P1-08, que substitui a rota). Mostra o pack com os dados de
 * `pack_get`, o aviso de higiene que "Agora não" deixou para depois (T04; até a seção Exportar
 * existir, ele aparece aqui) e diz com clareza que as seções ainda não chegaram.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { HygieneFinding, PackId, PackRow } from '../../../lib/ipc/bindings';
import { useHygieneFix, useHygieneScan, usePack } from '../api';
import { HygieneTable } from './HygieneTable';
import { useVersionsLine } from './PackTableRow';
import '../packs.css';

export function PackLanding({ packId }: { packId: PackId }) {
  const { t } = useTranslation('packs');
  const pack = usePack(packId);
  const back = { to: '/packs', label: t('pack.voltar') } as const;

  if (pack.isPending) {
    return (
      <AppPage back={back}>
        <LoadingState label={t('pack.carregando')} />
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
  return (
    <AppPage back={back} where={pack.data.name}>
      <PackSummary row={pack.data} />
    </AppPage>
  );
}

function PackSummary({ row }: { row: PackRow }) {
  const { t } = useTranslation('packs');
  const versions = useVersionsLine(row);
  return (
    <div className="stack openpack">
      <PageHead title={row.name} sub={row.status === 'ready' ? versions : null} />
      {row.status === 'folderMissing' ? (
        <Alert kind="danger" title={t('pack.pastaSumiu')}>
          <div className="alert__text path">{row.path}</div>
        </Alert>
      ) : null}
      {row.status === 'invalidPack' ? (
        <Alert kind="danger" title={t('pack.ilegivel')}>
          {row.detail ? <pre className="errpanel__pre">{row.detail}</pre> : null}
        </Alert>
      ) : null}
      <div className="panel">
        <dl className="kv">
          <dt>{t('pack.versao')}</dt>
          <dd className="t-mono">{row.version ?? t('lista.semData')}</dd>
          <dt>{t('pack.naoSalvas')}</dt>
          <dd>{row.unsavedFiles}</dd>
          <dt>{t('pack.pasta')}</dt>
          <dd className="path">{row.path}</dd>
        </dl>
      </div>
      {row.status === 'ready' && row.readOnlyReason === null ? (
        <PackHygiene packId={row.id} />
      ) : null}
      <EmptyState glyph="box" title={t('pack.editorTitulo')} text={t('pack.editorTexto')} />
    </div>
  );
}

/** Itens da higiene ainda no pack, com "Limpar N arquivos" (ponto de segurança antes). */
function PackHygiene({ packId }: { packId: PackId }) {
  const scan = useHygieneScan(packId, true);
  if (!scan.isSuccess || scan.data.length === 0) return null;
  return <HygieneCleanup key={scan.dataUpdatedAt} packId={packId} items={scan.data} />;
}

function HygieneCleanup({ packId, items }: { packId: PackId; items: HygieneFinding[] }) {
  const { t } = useTranslation('packs');
  const fix = useHygieneFix();
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => new Set(items.map((item) => item.path)),
  );
  return (
    <div className="stack-3">
      <HygieneTable items={items} selected={selected} onSelectedChange={setSelected} />
      {fix.isError ? <ErrorPanel compact error={fix.error} /> : null}
      <div className="btn-row btn-row--end">
        <Button
          variant="primary"
          loading={fix.isPending}
          disabled={selected.size === 0}
          onClick={() => {
            fix.mutate(
              { packId, paths: [...selected] },
              {
                onSuccess: (deleted) => {
                  showToast({ kind: 'ok', title: t('abrir.limpos', { count: deleted.length }) });
                },
              },
            );
          }}
        >
          {fix.isPending ? t('pack.limpando') : t('pack.limpar', { count: selected.size })}
        </Button>
      </div>
    </div>
  );
}
