/**
 * Achados do pré-teste na seção Problemas (SPEC T14 item 2; protótipo `problemas`): agrupados
 * por gravidade (Erro, Aviso, Informação), com as correções e "Ignorar neste pack". Os
 * ignorados ficam escondidos atrás de "Mostrar os ignorados", marcados "ignorado por você".
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { ReportFinding, Severity } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useIgnoreFinding } from '../api';
import { useFixes } from '../fixes';
import type { ProblemsBlockProps } from '../problems-blocks';
import { FindingCard } from './FindingCard';

const GROUPS: readonly Severity[] = ['error', 'warning', 'info'];

export function FindingsBlock({ packId, report, checking }: ProblemsBlockProps) {
  const { t } = useTranslation('diagnostico');
  const fixes = useFixes(packId);
  const ignore = useIgnoreFinding(packId);
  const [showIgnored, setShowIgnored] = useState(false);

  if (report === undefined) {
    return null;
  }
  if (report === null) {
    return checking ? (
      <section className="panel diag-progress" aria-label={t('secao.verificando')}>
        <ProgressBar
          value={null}
          label={t('secao.verificando')}
          meta={t('secao.verificandoMeta')}
        />
      </section>
    ) : null;
  }
  if (!report.checkedMods) {
    return (
      <EmptyState
        kind="ok"
        glyph="check"
        title={t('secao.semLoaderTitulo')}
        text={t('secao.semLoaderTexto')}
      />
    );
  }

  const ignored = report.findings.filter((entry) => entry.ignored);
  const visible = report.findings.filter((entry) => showIgnored || !entry.ignored);
  const onIgnore = (entry: ReportFinding, value: boolean) => {
    ignore.mutate(
      { findingId: entry.id, ignored: value },
      {
        onSuccess: () => {
          showToast({
            kind: 'ok',
            title: value ? t('achado.ignoradoToast') : t('achado.voltouToast'),
          });
        },
        onError: (error) => {
          showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        },
      },
    );
  };

  return (
    <div className="stack-3 diag-findings">
      {visible.length === 0 ? (
        <EmptyState
          kind="ok"
          glyph="check"
          title={t('secao.vazioTitulo')}
          text={t('secao.vazioTexto', { count: report.itemCount })}
        />
      ) : (
        GROUPS.map((severity) => {
          const entries = visible.filter((entry) => entry.finding.severity === severity);
          if (entries.length === 0) return null;
          const titleId = `diag-group-${severity}`;
          return (
            <section key={severity} aria-labelledby={titleId}>
              <h2 className="group-title" id={titleId}>
                {t(`grupos.${severity}`)} <span className="t-3">{t(`grupos.${severity}Nota`)}</span>
              </h2>
              <div>
                {entries.map((entry) => (
                  <FindingCard
                    key={entry.id}
                    packId={packId}
                    entry={entry}
                    onFix={fixes.run}
                    busyFix={fixes.busy}
                    onIgnore={onIgnore}
                    ignoring={ignore.isPending && ignore.variables.findingId === entry.id}
                  />
                ))}
              </div>
            </section>
          );
        })
      )}
      {ignored.length > 0 ? (
        <p className="t-sm t-3 row row--gap-3">
          <span>{t('secao.ignorados', { count: ignored.length })}</span>
          <Button
            variant="link"
            size="sm"
            aria-expanded={showIgnored}
            onClick={() => {
              setShowIgnored((open) => !open);
            }}
          >
            {showIgnored ? t('secao.esconderIgnorados') : t('secao.mostrarIgnorados')}
          </Button>
        </p>
      ) : null}
      {fixes.dialogs}
    </div>
  );
}
