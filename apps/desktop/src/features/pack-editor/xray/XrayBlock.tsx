/**
 * "O que este mod altera no jogo" nos detalhes do item (SPEC T07; CA-T07-04 e CA-T07-05; D-10;
 * protótipo `mods-raio-x`): o resumo ("Altera 214 partes do jogo, em 3 configs. 2 também
 * alteradas por outros mods."), as sobreposições com outros mods (risco com selo e palavra e o
 * motivo em linguagem simples), "Ver todas as alterações" no mesmo painel e "Ver em Problemas"
 * quando o pack tem o aviso. Nunca "são incompatíveis": "alteram o mesmo ponto do jogo".
 */
import { Link } from '@tanstack/react-router';
import { List } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type { ModReport, PackId, XrayModView } from '../../../lib/ipc/bindings';
import { useDiagnosticsReport } from '../../diagnostics/api';
import type { DetailBlockProps } from '../details/blocks';
import { useXray } from './api';
import { OverlapItem } from './OverlapItem';
import { XrayAll } from './XrayAll';
import './xray.css';

/** Sobreposições mostradas no resumo (o resto em "Ver todas as alterações"). */
export const SUMMARY_OVERLAPS = 5;

export function XrayBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('raioX');
  const headingId = useId();
  const applies = item.kind === 'mod' && item.state === 'ok';
  const xray = useXray(packId, item.path, applies);
  if (!applies) {
    return null;
  }
  return (
    <section className="xray" aria-labelledby={headingId}>
      <h3 className="field__label" id={headingId}>
        {t('titulo')}
      </h3>
      {xray.error ? (
        <ErrorPanel
          compact
          error={xray.error}
          onRetry={() => {
            void xray.refetch();
          }}
        />
      ) : xray.data ? (
        <XrayContent packId={packId} path={item.path} modName={item.name} view={xray.data} />
      ) : (
        <LoadingState inline label={t('carregando')} />
      )}
    </section>
  );
}

function XrayContent({
  packId,
  path,
  modName,
  view,
}: {
  packId: PackId;
  path: string;
  modName: string;
  view: XrayModView;
}) {
  const { t } = useTranslation('raioX');
  const report = view.report;
  if (view.state === 'noJar' || !report) {
    return <p className="t-sm t-3">{t('semJar')}</p>;
  }
  return (
    <>
      <Notes report={report} nativeLoader={view.nativeLoader} />
      {report.changeCount === 0 ? (
        <p className="t-sm t-2">{t('semAlteracoes')}</p>
      ) : (
        <Summary packId={packId} path={path} modName={modName} report={report} names={view.names} />
      )}
    </>
  );
}

function Notes({ report, nativeLoader }: { report: ModReport; nativeLoader: boolean }) {
  const { t } = useTranslation('raioX');
  const notes: string[] = [];
  if (report.connector && nativeLoader) notes.push(t('connector'));
  if (report.mixinSquared && report.changeCount > 0) notes.push(t('mixinSquared'));
  if (report.disabledClassCount > 0) {
    notes.push(t('desligadas', { count: report.disabledClassCount }));
  }
  if (report.issues.length > 0) {
    notes.push(t('problemasDeLeitura', { count: report.issues.length }));
  }
  return notes.map((note) => (
    <p key={note} className="t-xs t-3 xray__note">
      {note}
    </p>
  ));
}

function Summary({
  packId,
  path,
  modName,
  report,
  names,
}: {
  packId: PackId;
  path: string;
  modName: string;
  report: ModReport;
  names: Partial<Record<string, string>>;
}) {
  const { t } = useTranslation('raioX');
  const [all, setAll] = useState(false);
  const diagnostics = useDiagnosticsReport(packId, { refetchOnMount: false });
  const inProblems = (diagnostics.data?.findings ?? []).some(
    (entry) => entry.finding.rule === 'W_MIXIN_OVERLAP' && entry.items.some((i) => i.path === path),
  );
  const shown = report.overlaps.slice(0, SUMMARY_OVERLAPS);
  const hidden = report.overlaps.length - shown.length;
  return (
    <>
      <p className="t-sm xray__summary">
        {t('resumo', { count: report.pointCount })}
        {t('emConfigs', { count: report.configs.length })}
        {report.sharedPointCount > 0 ? (
          <>
            {' '}
            <b>{t('compartilhadas', { count: report.sharedPointCount })}</b>
          </>
        ) : null}
      </p>
      {shown.length > 0 ? (
        <ul className="mixins">
          {shown.map((overlap) => (
            <OverlapItem
              key={`${overlap.targetClass}#${overlap.method}`}
              overlap={overlap}
              names={names}
            />
          ))}
        </ul>
      ) : null}
      {hidden > 0 ? <p className="t-xs t-3">{t('maisSobreposicoes', { count: hidden })}</p> : null}
      <div className="row row--wrap xray__actions">
        <XrayAll
          open={all}
          onOpenChange={setAll}
          trigger={
            <Button size="sm" icon={List}>
              {t('verTodas')}
            </Button>
          }
          modName={modName}
          report={report}
          names={names}
        />
        {inProblems ? (
          <Button size="sm" variant="ghost" asChild>
            <Link to="/packs/$packId/problemas" params={{ packId }}>
              {t('verEmProblemas')}
            </Link>
          </Button>
        ) : null}
      </div>
      <p className="t-xs t-3">{t('rodape')}</p>
    </>
  );
}
