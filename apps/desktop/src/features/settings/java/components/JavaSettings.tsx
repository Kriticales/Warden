/**
 * Seção Java de Configurações → Teste (SPEC T21; CA-T21-04; ADR-0029): a tabela com cada Java
 * instalado pelo Warden, os packs que usam cada um e por que aquela versão, com "Procurar
 * atualizações do Java" e "Remover Javas sem uso".
 *
 * A P1-13 encaixa este bloco no painel "Teste" da página de Configurações (registro
 * `features/settings/sections.ts`).
 */
import { RefreshCw, Trash2 } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { LoadingState } from '../../../../components/common/LoadingState';
import { Alert } from '../../../../components/ui/alert';
import { Button } from '../../../../components/ui/button';
import { showToast } from '../../../../components/ui/toast';
import { Tooltip } from '../../../../components/ui/tooltip';
import type { JavaOverview, UpdateReport } from '../../../../lib/ipc/bindings';
import {
  useCheckJavaUpdates,
  useJavaRuntimes,
  useRemoveJava,
  useRemoveUnusedJavas,
} from '../hooks/useJavaRuntimes';
import {
  buildRows,
  joinNames,
  reasonLabel,
  unusedCount,
  versionText,
  type JavaTableRow,
} from '../lib/java-table';

export interface JavaSettingsProps {
  /** Nível do título, para seguir a hierarquia da página. Padrão: 3 (dentro de "Teste"). */
  headingLevel?: 3 | 4;
}

export function JavaSettings({ headingLevel = 3 }: JavaSettingsProps) {
  const { t } = useTranslation('java');
  const titleId = useId();
  const runtimes = useJavaRuntimes();
  const Heading = headingLevel === 3 ? 'h3' : 'h4';

  let body;
  if (runtimes.isPending) {
    body = <LoadingState inline label={t('secao.carregando')} />;
  } else if (runtimes.isError) {
    body = (
      <ErrorPanel
        compact
        error={runtimes.error}
        onRetry={() => {
          void runtimes.refetch();
        }}
      />
    );
  } else {
    body = <JavaContent overview={runtimes.data} />;
  }

  return (
    <div role="group" aria-labelledby={titleId}>
      <Heading className="field__label" id={titleId}>
        {t('secao.titulo')}
      </Heading>
      <p className="t-sm t-2 mt-1">{t('secao.descricao')}</p>
      <div className="stack-3 mt-2">{body}</div>
    </div>
  );
}

function JavaContent({ overview }: { overview: JavaOverview }) {
  const { t } = useTranslation('java');
  const rows = buildRows(overview);
  return (
    <>
      {rows.length === 0 ? (
        <p className="t-sm t-3">{t('secao.vazio')}</p>
      ) : (
        <JavaTable rows={rows} />
      )}
      {overview.unresolved.length > 0 ? (
        <Alert kind="warn" compact title={t('semDecisao.titulo')}>
          <p className="t-sm">
            {t('semDecisao.texto', {
              packs: joinNames(overview.unresolved.map((pack) => pack.name)),
            })}
          </p>
        </Alert>
      ) : null}
      {overview.brokenCount > 0 ? (
        <p className="t-sm t-3">{t('pastasQuebradas', { count: overview.brokenCount })}</p>
      ) : null}
      <JavaActions overview={overview} />
    </>
  );
}

function JavaTable({ rows }: { rows: JavaTableRow[] }) {
  const { t } = useTranslation('java');
  return (
    <div className="tablewrap">
      <table className="table">
        <caption className="sr-only">{t('tabela.legenda')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('tabela.colunaJava')}</th>
            <th scope="col">{t('tabela.colunaUsadoPor')}</th>
            <th scope="col">{t('tabela.colunaMotivo')}</th>
            <th scope="col" className="shrink">
              <span className="sr-only">{t('tabela.colunaAcoes')}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <JavaRow key={row.key} row={row} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function JavaRow({ row }: { row: JavaTableRow }) {
  const { t } = useTranslation('java');
  const { runtime } = row;
  return (
    <tr>
      <th scope="row" className="t-strong">
        <div>{t('tabela.java', { major: row.major })}</div>
        <div className="t-sm t-3">
          {runtime
            ? t('tabela.detalhesVersao', {
                version: versionText(runtime.version),
                source:
                  runtime.source === 'temurin' ? t('tabela.fonteTemurin') : t('tabela.fonteMojang'),
              })
            : t('tabela.naoBaixado')}
        </div>
      </th>
      <td>
        {row.packs.length === 0 ? (
          <span className="t-3">{t('tabela.nenhumPack')}</span>
        ) : (
          <ul>
            {row.packs.map((pack) => (
              <li key={pack.packId}>
                {t('tabela.packComVersao', { name: pack.name, minecraft: pack.minecraft })}
              </li>
            ))}
          </ul>
        )}
        {row.inGame ? <div className="t-sm t-3">{t('tabela.emJogo')}</div> : null}
        {row.state === 'naoBaixado' ? (
          <div className="t-sm t-3">{t('tabela.naoBaixadoDetalhe')}</div>
        ) : null}
      </td>
      <td className="t-2">
        <RowReasons row={row} />
      </td>
      <td className="shrink">{runtime ? <RemoveJava row={row} /> : null}</td>
    </tr>
  );
}

function RowReasons({ row }: { row: JavaTableRow }) {
  const { t } = useTranslation('java');
  if (row.state === 'substituido') {
    return <p>{t('tabela.substituido', { version: row.supersededBy ?? '' })}</p>;
  }
  if (row.state === 'semUso') {
    return <p>{t('tabela.semUso')}</p>;
  }
  return (
    <>
      {row.reasons.map((reason) => (
        <p key={`${reason.key}-${JSON.stringify(reason.params)}`}>{reasonLabel(t, reason)}</p>
      ))}
    </>
  );
}

function RemoveJava({ row }: { row: JavaTableRow }) {
  const { t } = useTranslation('java');
  const remove = useRemoveJava();
  const runtime = row.runtime;
  if (!runtime) return null;
  const version = versionText(runtime.version);
  const label = t('tabela.removerRotulo', { version });
  const packs = row.packs.map((pack) => pack.name);
  const trigger = (
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      icon={Trash2}
      aria-label={label}
      disabled={row.inGame}
    />
  );
  if (row.inGame) {
    return (
      <Tooltip content={t('tabela.emJogo')}>
        <span>{trigger}</span>
      </Tooltip>
    );
  }
  return (
    <ConfirmDialog
      trigger={trigger}
      title={t('confirmarRemocao.titulo', { version })}
      description={
        packs.length === 0
          ? t('confirmarRemocao.textoSemPacks')
          : t('confirmarRemocao.textoComPacks', { count: packs.length, packs: joinNames(packs) })
      }
      confirmLabel={t('confirmarRemocao.confirmar')}
      confirmingLabel={t('acoes.removendo')}
      cancelLabel={t('confirmarRemocao.cancelar')}
      onConfirm={async () => {
        await remove.mutateAsync(runtime.id);
        showToast({ kind: 'ok', title: t('resultado.removidos', { count: 1 }) });
      }}
    />
  );
}

function updateToast(report: UpdateReport, t: ReturnType<typeof useTranslation<'java'>>['t']) {
  const lines: string[] = [];
  if (report.updated.length > 0) {
    lines.push(t('resultado.atualizados', { count: report.updated.length }));
    if (report.updated.some((update) => !update.oldRemoved)) {
      lines.push(t('resultado.antigoMantido'));
    }
  }
  if (report.failed.length > 0) {
    lines.push(t('resultado.falhou', { count: report.failed.length }));
  }
  if (lines.length === 0) {
    showToast({ kind: 'ok', title: t('resultado.nadaNovo') });
    return;
  }
  const [title, ...rest] = lines;
  showToast({
    kind: report.failed.length > 0 ? 'warn' : 'ok',
    title: title ?? '',
    ...(rest.length > 0 ? { text: rest.join(' ') } : {}),
  });
}

function JavaActions({ overview }: { overview: JavaOverview }) {
  const { t } = useTranslation('java');
  const check = useCheckJavaUpdates();
  const removeUnused = useRemoveUnusedJavas();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const unused = unusedCount(overview);
  const nothingToRemove = unused === 0 && overview.brokenCount === 0;

  return (
    <>
      <div className="btn-row">
        <Button
          size="sm"
          icon={RefreshCw}
          loading={check.isPending}
          onClick={() => {
            check.mutate(undefined, {
              onSuccess: (report) => {
                updateToast(report, t);
              },
            });
          }}
        >
          {check.isPending ? t('acoes.procurando') : t('acoes.procurarAtualizacoes')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            if (nothingToRemove) {
              showToast({ kind: 'info', title: t('resultado.nadaRemovido') });
            } else {
              setConfirmOpen(true);
            }
          }}
        >
          {t('acoes.removerSemUso')}
        </Button>
      </div>
      {check.isError ? <ErrorPanel compact error={check.error} /> : null}
      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title={t('confirmarSemUso.titulo')}
        description={
          unused > 0
            ? t('confirmarSemUso.texto', { count: unused })
            : t('confirmarSemUso.textoSoPastas')
        }
        confirmLabel={t('confirmarSemUso.confirmar')}
        confirmingLabel={t('acoes.removendo')}
        cancelLabel={t('confirmarSemUso.cancelar')}
        onConfirm={async () => {
          const before = overview.runtimes.length;
          const after = await removeUnused.mutateAsync(undefined);
          const removed = before - after.runtimes.length;
          showToast({
            kind: 'ok',
            title:
              removed > 0
                ? t('resultado.removidos', { count: removed })
                : t('resultado.nadaRemovido'),
          });
        }}
      />
    </>
  );
}
