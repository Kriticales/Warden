/**
 * Revisão antes de atualizar (SPEC T10; protótipo `updateDialog`): cada item com a versão de
 * agora e a nova, o canal, as novidades recolhidas e uma caixa marcada; as dependências novas e
 * as incompatibilidades novas; e o ponto de segurança, quando são vários itens. Nada muda até
 * confirmar. Serve para um item (Atualizar) e para vários (Revisar e atualizar, Atualizar
 * selecionados).
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { SafeHtml } from '../../components/common/SafeHtml';
import { SafeMarkdown } from '../../components/common/SafeMarkdown';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogContent } from '../../components/ui/dialog';
import { showToast } from '../../components/ui/toast';
import type {
  AppliedUpdates,
  ChangelogEntry,
  InventoryItem,
  PackId,
  PlanItem,
  UpdatePlan,
} from '../../lib/ipc/bindings';
import { useApplyUpdates, useUpdatePlan } from './api';

export interface ReviewDialogProps {
  packId: PackId;
  /** Os itens a revisar (todos com atualização disponível); `null` = fechado. */
  items: readonly InventoryItem[] | null;
  onClose: () => void;
  /** Depois de atualizar (limpa a seleção). */
  onApplied?: (applied: AppliedUpdates) => void;
}

export function ReviewDialog({ packId, items, onClose, onApplied }: ReviewDialogProps) {
  const paths = items ? items.map((item) => item.path) : null;
  return (
    <Dialog
      open={items !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {items && paths ? (
        <ReviewBody
          // Outro conjunto de itens recomeça a revisão (caixas marcadas de novo).
          key={paths.join('\n')}
          packId={packId}
          paths={paths}
          onClose={onClose}
          {...(onApplied ? { onApplied } : {})}
        />
      ) : null}
    </Dialog>
  );
}

function ReviewBody({
  packId,
  paths,
  onClose,
  onApplied,
}: {
  packId: PackId;
  paths: string[];
  onClose: () => void;
  onApplied?: (applied: AppliedUpdates) => void;
}) {
  const { t } = useTranslation('atualizacoes');
  const plan = useUpdatePlan(packId, paths);
  const apply = useApplyUpdates(packId);
  const [unchecked, setUnchecked] = useState<ReadonlySet<string>>(new Set());
  const items = plan.data?.items ?? [];
  const chosen = items.filter((item) => !unchecked.has(item.path));
  const single = paths.length === 1 ? items[0] : undefined;

  const confirm = () => {
    apply.mutate(
      chosen.map((item) => ({ path: item.path, toVersionId: item.newVersion.id })),
      {
        onSuccess: (applied) => {
          showToast({
            kind: 'ok',
            title:
              applied.updated.length === 1 && applied.updated[0]
                ? t('revisao.concluidoUm', {
                    name: applied.updated[0].name,
                    version: applied.updated[0].to,
                  })
                : t('revisao.concluidoVarios', { count: applied.updated.length }),
          });
          onApplied?.(applied);
          onClose();
        },
      },
    );
  };

  return (
    <DialogContent
      size="lg"
      title={
        paths.length === 1
          ? t('revisao.tituloUm', { name: single?.name ?? '' })
          : t('revisao.tituloVarios', { count: paths.length })
      }
      description={t('revisao.subtitulo')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('revisao.cancelar')}
          </Button>
          <Button
            variant="primary"
            disabled={plan.isPending || chosen.length === 0}
            loading={apply.isPending}
            title={chosen.length === 0 && !plan.isPending ? t('revisao.nenhumMarcado') : undefined}
            onClick={confirm}
          >
            {apply.isPending
              ? t('revisao.confirmando')
              : chosen.length === 1 && chosen[0]
                ? t('revisao.confirmarUm', { name: chosen[0].name })
                : t('revisao.confirmarVarios', { count: chosen.length })}
          </Button>
        </>
      }
    >
      {plan.isPending ? <LoadingState inline label={t('revisao.carregando')} /> : null}
      {plan.isError ? (
        <ErrorPanel
          compact
          error={plan.error}
          onRetry={() => {
            void plan.refetch();
          }}
        />
      ) : null}
      {plan.data ? (
        <div className="stack-3">
          <Conflicts plan={plan.data} />
          <NewDependencies plan={plan.data} />
          <div className="tablewrap">
            <table className="table table--plain">
              <thead>
                <tr>
                  <th className="shrink">
                    <span className="sr-only">{t('revisao.incluir', { name: '' })}</span>
                  </th>
                  <th>{t('revisao.item')}</th>
                  <th>{t('revisao.agora')}</th>
                  <th>{t('revisao.nova')}</th>
                </tr>
              </thead>
              <tbody>
                {plan.data.items.map((item) => (
                  <ReviewRow
                    key={item.path}
                    item={item}
                    checked={!unchecked.has(item.path)}
                    onToggle={() => {
                      setUnchecked((current) => {
                        const next = new Set(current);
                        if (next.has(item.path)) next.delete(item.path);
                        else next.add(item.path);
                        return next;
                      });
                    }}
                  />
                ))}
              </tbody>
            </table>
          </div>
          {paths.length > 1 ? <p className="t-sm t-3">{t('revisao.pontoDeSeguranca')}</p> : null}
        </div>
      ) : null}
      {apply.isError ? <ErrorPanel compact error={apply.error} /> : null}
    </DialogContent>
  );
}

function ReviewRow({
  item,
  checked,
  onToggle,
}: {
  item: PlanItem;
  checked: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation('atualizacoes');
  return (
    <>
      <tr>
        <td>
          <label className="check">
            <input
              type="checkbox"
              checked={checked}
              aria-label={t('revisao.incluir', { name: item.name })}
              onChange={onToggle}
            />
          </label>
        </td>
        <td className="t-strong">{item.name}</td>
        <td className="t-mono t-2">{item.current ?? ''}</td>
        <td>
          <span className="t-mono t-primary">{item.newVersion.number}</span>
          {item.newVersion.channel !== 'release' ? (
            <>
              {' '}
              <span className="tag tag--warn">{t(`revisao.canal.${item.newVersion.channel}`)}</span>
            </>
          ) : null}
        </td>
      </tr>
      <tr>
        <td />
        <td colSpan={3}>
          {item.manualDownload ? <p className="t-xs t-3">{t('revisao.downloadManual')}</p> : null}
          <details className="disclosure">
            <summary>{t('revisao.verNovidades')}</summary>
            <Changelog entries={item.changelog} />
          </details>
        </td>
      </tr>
    </>
  );
}

function Changelog({ entries }: { entries: readonly ChangelogEntry[] }) {
  const { t } = useTranslation('atualizacoes');
  const withText = entries.filter((entry) => entry.text?.trim());
  if (withText.length === 0) {
    return <p className="t-sm t-3">{t('revisao.semNovidades')}</p>;
  }
  return (
    <div className="stack-2">
      {withText.map((entry) => (
        <section key={entry.version}>
          <h4 className="t-sm t-strong t-mono">{entry.version}</h4>
          {entry.format === 'html' ? (
            <SafeHtml className="prose" html={entry.text ?? ''} />
          ) : (
            <SafeMarkdown className="prose">{entry.text ?? ''}</SafeMarkdown>
          )}
        </section>
      ))}
    </div>
  );
}

function NewDependencies({ plan }: { plan: UpdatePlan }) {
  const { t } = useTranslation('atualizacoes');
  if (plan.newDependencies.length === 0) return null;
  return (
    <Alert
      kind="warn"
      compact
      title={t('revisao.dependenciasTitulo', { count: plan.newDependencies.length })}
    >
      <div className="alert__text">{t('revisao.dependenciasTexto')}</div>
      <ul>
        {plan.newDependencies.map((dependency) => (
          <li key={`${dependency.source}:${dependency.projectId}`}>
            {t('revisao.dependenciaLinha', {
              name: dependency.name,
              por: dependency.neededBy.join(', '),
            })}
          </li>
        ))}
      </ul>
    </Alert>
  );
}

function Conflicts({ plan }: { plan: UpdatePlan }) {
  const { t } = useTranslation('atualizacoes');
  if (plan.conflicts.length === 0) return null;
  return (
    <Alert
      kind="warn"
      compact
      title={t('revisao.conflitosTitulo', { count: plan.conflicts.length })}
    >
      <ul>
        {plan.conflicts.map((conflict) => (
          <li key={`${conflict.item}:${conflict.with}`}>
            {t('revisao.conflitoLinha', { item: conflict.item, com: conflict.with })}
          </li>
        ))}
      </ul>
    </Alert>
  );
}
