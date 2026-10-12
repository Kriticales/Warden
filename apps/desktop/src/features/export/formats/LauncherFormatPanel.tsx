/**
 * Formatos de outros launchers (SPEC T19, P1): no lugar de "3. O que vai no pack" e do botão
 * Exportar…, a tela explica o que o formato perde, pede as decisões (trocar pelo Modrinth,
 * embutir arquivos de terceiros, a versão do pack) e só então gera. O pack no Warden não muda.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import type { LauncherFormat, PackId } from '../../../lib/ipc/bindings';
import { useFormatAnalysis } from './api';
import { DecisionsPanel } from './DecisionsPanel';
import { FormatAction } from './FormatAction';
import { LossesPanel } from './LossesPanel';
import { NO_DECISIONS, type FormatDecisions } from './model';
import { SummaryPanel } from './SummaryPanel';
import { VersionPanel } from './VersionPanel';

export function LauncherFormatPanel({
  packId,
  format,
  diagnosticErrors,
  readOnly,
}: {
  packId: PackId;
  format: LauncherFormat;
  diagnosticErrors: number | null;
  readOnly: boolean;
}) {
  const { t } = useTranslation('exportarFormatos');
  const analysis = useFormatAnalysis(packId, format);
  const [decisions, setDecisions] = useState<FormatDecisions>(NO_DECISIONS);
  const update = (patch: Partial<FormatDecisions>) => {
    setDecisions((current) => ({ ...current, ...patch }));
  };

  if (analysis.isPending) return <LoadingState label={t('carregando')} />;
  if (analysis.isError) {
    return (
      <ErrorPanel
        title={t('erroAnalise')}
        error={analysis.error}
        onRetry={() => {
          void analysis.refetch();
        }}
      />
    );
  }
  const data = analysis.data;
  return (
    <>
      <SummaryPanel analysis={data} decisions={decisions} />
      <LossesPanel analysis={data} />
      {data.version === null ? (
        <VersionPanel
          value={decisions.version}
          onChange={(version) => {
            update({ version });
          }}
        />
      ) : null}
      {data.items.length > 0 ? (
        <DecisionsPanel analysis={data} decisions={decisions} format={format} onChange={update} />
      ) : null}
      <FormatAction
        packId={packId}
        format={format}
        analysis={data}
        decisions={decisions}
        diagnosticErrors={diagnosticErrors}
        readOnly={readOnly}
      />
    </>
  );
}
