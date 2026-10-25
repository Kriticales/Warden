/**
 * "Achados antes do teste" na seção Problemas (esboço aprovado do bug-01, "Separadas"; SPEC T14
 * item 2): um painel com os contadores na barra e os achados do pré-teste em linhas, agrupados
 * por gravidade (Erros, Avisos, Informações), com as correções, **✦ Perguntar ao Sculk** e
 * "Ignorar neste pack". Os ignorados ficam escondidos atrás de "Mostrar os ignorados",
 * marcados "ignorado por você".
 */
import { TriangleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { ReportFinding, Severity } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useIgnoreFinding } from '../api';
import { useFixes } from '../fixes';
import type { ProblemsBlockProps } from '../problems-blocks';
import { countBySeverity } from '../text';
import { Count } from './Count';
import { FindingRow } from './FindingRow';

const GROUPS: readonly Severity[] = ['error', 'warning', 'info'];

export function FindingsBlock({ packId, report, checking }: ProblemsBlockProps) {
  const { t } = useTranslation('diagnostico');
  const titleId = useId();
  const fixes = useFixes(packId);
  const ignore = useIgnoreFinding(packId);
  const [showIgnored, setShowIgnored] = useState(false);

  if (report === undefined) {
    return null;
  }

  const counts = report ? countBySeverity(report.findings) : null;
  const bar = (
    <div className="panel__bar">
      <h2 className="panel__bar-title" id={titleId}>
        <Icon icon={TriangleAlert} />
        {t('antesDoTeste.titulo')}
      </h2>
      {counts && counts.error > 0 ? (
        <Count
          kind="danger"
          value={counts.error}
          label={t('antesDoTeste.erros', { count: counts.error })}
        />
      ) : null}
      {counts && counts.warning > 0 ? (
        <Count
          kind="warn"
          value={counts.warning}
          label={t('antesDoTeste.avisos', { count: counts.warning })}
        />
      ) : null}
      {counts && counts.info > 0 ? (
        <Count value={counts.info} label={t('antesDoTeste.infos', { count: counts.info })} />
      ) : null}
    </div>
  );

  let content;
  if (report === null) {
    content = (
      <div className="panel__body">
        {checking ? (
          <ProgressBar
            value={null}
            label={t('secao.verificando')}
            meta={t('secao.verificandoMeta')}
          />
        ) : (
          <p className="t-sm t-3">{t('secao.nunca')}</p>
        )}
      </div>
    );
  } else if (!report.checkedMods) {
    content = (
      <div className="panel__body">
        <EmptyState
          kind="ok"
          glyph="check"
          compact
          headingLevel={3}
          title={t('secao.semLoaderTitulo')}
          text={t('secao.semLoaderTexto')}
        />
      </div>
    );
  } else {
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
    content = (
      <>
        {report.pass === 'quick' ? (
          <div className="panel__body">
            <Alert kind="neutral" compact>
              {t('secao.rapida')}
            </Alert>
          </div>
        ) : null}
        {visible.length === 0 ? (
          <div className="panel__body">
            <EmptyState
              kind="ok"
              glyph="check"
              compact
              headingLevel={3}
              title={t('secao.vazioTitulo')}
              text={t('secao.vazioTexto', { count: report.itemCount })}
            />
          </div>
        ) : (
          GROUPS.map((severity) => {
            const entries = visible.filter((entry) => entry.finding.severity === severity);
            if (entries.length === 0) return null;
            const groupId = `${titleId}-${severity}`;
            return (
              <section key={severity} aria-labelledby={groupId}>
                <h3 className="rowlist__sev" id={groupId}>
                  {t(`antesDoTeste.grupo.${severity}`)}
                </h3>
                <ul className="rowlist">
                  {entries.map((entry) => (
                    <FindingRow
                      key={entry.id}
                      packId={packId}
                      entry={entry}
                      onFix={fixes.run}
                      busyFix={fixes.busy}
                      onIgnore={onIgnore}
                      ignoring={ignore.isPending && ignore.variables.findingId === entry.id}
                    />
                  ))}
                </ul>
              </section>
            );
          })
        )}
        <div className="panel__foot">
          {ignored.length > 0 ? (
            <>
              <span className="t-3">{t('secao.ignorados', { count: ignored.length })}</span>
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
            </>
          ) : (
            <span className="t-xs t-3">{t('secao.tambem')}</span>
          )}
        </div>
      </>
    );
  }

  return (
    <section className="panel panel--parts diag-findings" aria-labelledby={titleId}>
      {bar}
      {content}
      {fixes.dialogs}
    </section>
  );
}
