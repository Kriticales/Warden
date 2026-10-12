/**
 * Seção Exportar do pack (SPEC T19; protótipo `exportar`): uma página de cima para baixo —
 * antes de exportar → formato → o que vai no pack → Exportar…
 *
 * A prévia vem de `export_preview` (os mesmos bytes que a exportação vai levar); o destino é
 * escolhido no diálogo nativo do Rust ao clicar em Exportar… (ARCHITECTURE §20).
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import type { PackId } from '../../lib/ipc/bindings';
import { usePack } from '../packs/api';
import { useExportPreview } from './api';
import { ContentPanel } from './components/ContentPanel';
import { ExportAction } from './components/ExportAction';
import { FormatPanel } from './components/FormatPanel';
import { LauncherFormatPanel } from './formats/LauncherFormatPanel';
import { isLauncherFormat, type UiFormat } from './model';
import { PreflightPanel } from './components/PreflightPanel';
import './export.css';

export function ExportPage({ packId }: { packId: PackId }) {
  const { t } = useTranslation('exportar');
  const pack = usePack(packId);
  const preview = useExportPreview(packId);
  const [format, setFormat] = useState<UiFormat>('folder');
  const readOnly = (pack.data?.readOnlyReason ?? null) !== null;

  return (
    <div className="stack export-page">
      <PageHead title={t('titulo')} sub={t('sub')} />
      {preview.isPending ? (
        <LoadingState label={t('carregando')} />
      ) : preview.isError ? (
        <ErrorPanel
          title={t('erroPrevia')}
          error={preview.error}
          onRetry={() => {
            void preview.refetch();
          }}
        />
      ) : (
        <>
          <PreflightPanel packId={packId} preflight={preview.data.preflight} readOnly={readOnly} />
          <FormatPanel value={format} onChange={setFormat} />
          {isLauncherFormat(format) ? (
            <LauncherFormatPanel
              key={format}
              packId={packId}
              format={format}
              diagnosticErrors={preview.data.preflight.diagnosticErrors}
              readOnly={readOnly}
            />
          ) : (
            <>
              <ContentPanel packId={packId} preview={preview.data} readOnly={readOnly} />
              <ExportAction
                packId={packId}
                format={format}
                diagnosticErrors={preview.data.preflight.diagnosticErrors}
                readOnly={readOnly}
              />
            </>
          )}
        </>
      )}
    </div>
  );
}
