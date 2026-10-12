/**
 * Seção Histórico do pack (SPEC T17; protótipo `historico`): a área de publicação para os
 * jogadores (V-03), as alterações não salvas no topo da linha do tempo e as versões salvas, com
 * o estado de cada uma. Sobre a linha do tempo ficam "Voltar para esta versão" e, no fim,
 * "Pontos de segurança…".
 */
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import type { PackId } from '../../lib/ipc/bindings';
import { usePack } from '../packs/api';
import { useHistory } from './api';
import { SafetyPointsDialog } from './components/SafetyPointsDialog';
import { SaveVersionFlow } from './components/SaveVersionDialog';
import { UnsavedItem } from './components/UnsavedItem';
import { VersionItem } from './components/VersionItem';
import { publicationSlots } from './publication';
import './versioning.css';

export function HistoryPage({ packId }: { packId: PackId }) {
  const { t } = useTranslation('versoes');
  const pack = usePack(packId);
  const history = useHistory(packId);
  const [saving, setSaving] = useState(false);
  const [showSafety, setShowSafety] = useState(false);
  const Publication = publicationSlots.panel;

  const readOnlyReason = pack.data?.readOnlyReason ?? null;
  const canWrite = readOnlyReason === null;

  let body: ReactNode;
  if (history.isError || pack.isError) {
    body = (
      <ErrorPanel
        title={t('historico.erro')}
        error={history.error ?? pack.error}
        onRetry={() => {
          void history.refetch();
        }}
      />
    );
  } else if (!history.isSuccess || !pack.isSuccess) {
    body = <LoadingState label={t('historico.carregando')} />;
  } else {
    const { unsaved, lastVersion, versions } = history.data;
    body = (
      <>
        <Publication packId={packId} pack={pack.data} versions={versions} />
        <h2 className="group-title">{t('historico.versoes')}</h2>
        <ol className="timeline">
          <UnsavedItem
            packId={packId}
            changes={unsaved}
            lastVersion={lastVersion}
            canWrite={canWrite}
            onSave={() => {
              setSaving(true);
            }}
          />
          {versions.map((version, index) => (
            <VersionItem
              key={version.version}
              packId={packId}
              version={version}
              defaultOpen={index === 0 && unsaved.files.length === 0}
              canWrite={canWrite}
              // A versão mais nova só é destino útil se o pack tem alterações a desfazer.
              canRestore={index > 0 || unsaved.files.length > 0}
              unsavedCount={unsaved.files.length}
            />
          ))}
        </ol>
        {versions.length === 0 ? (
          <EmptyState
            compact
            glyph="box"
            title={t('historico.semVersoes.titulo')}
            text={t('historico.semVersoes.texto')}
          />
        ) : null}
        <p className="t-sm">
          <Button
            variant="link"
            onClick={() => {
              setShowSafety(true);
            }}
          >
            {t('historico.pontosLink')}
          </Button>{' '}
          <span className="t-3">{t('historico.pontosDica')}</span>
        </p>
      </>
    );
  }

  return (
    <div className="stack history-page">
      <PageHead title={t('historico.titulo')} sub={t('historico.sub')} />
      {readOnlyReason !== null ? (
        <Alert kind="warn">{t('historico.somenteLeitura', { motivo: readOnlyReason })}</Alert>
      ) : null}
      {body}
      <SaveVersionFlow packId={packId} open={saving} onOpenChange={setSaving} />
      {showSafety ? (
        <SafetyPointsDialog
          packId={packId}
          canWrite={canWrite}
          onClose={() => {
            setShowSafety(false);
          }}
        />
      ) : null}
    </div>
  );
}
