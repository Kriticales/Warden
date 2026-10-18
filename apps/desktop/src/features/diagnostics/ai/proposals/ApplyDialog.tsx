/**
 * O diálogo de **Aplicar** uma proposta (SPEC T14 ✦ passo 3; CA-T14-12): a diferença exata,
 * preparada pelo mesmo serviço da interface (o plano de Adicionar com as dependências, o de
 * Remover, o de Trocar de versão, o lado, o texto da config com só aquela chave trocada, os
 * ajustes do teste antes e depois). Nada é gravado até o botão **Aplicar** do diálogo, que manda
 * o `token` do que foi mostrado: se o pack mudou no meio, nada é gravado e o erro diz para
 * revisar de novo. O foco volta para o botão que abriu (Radix).
 */
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../../components/common/DiffView';
import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { LoadingState } from '../../../../components/common/LoadingState';
import { Button } from '../../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../../components/ui/dialog';
import { showToast } from '../../../../components/ui/toast';
import type { PackId, Proposal, ProposalPreview } from '../../../../lib/ipc/bindings';
import { useApplyProposal, useProposalReview } from './api';
import { proposalTitle, settingsLine, sideLabel } from './text';

export interface ApplyDialogProps {
  packId: PackId;
  conversationId: string;
  proposal: Proposal;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ApplyDialog({
  packId,
  conversationId,
  proposal,
  open,
  onOpenChange,
}: ApplyDialogProps) {
  const { t } = useTranslation('iaPropostas');
  const review = useProposalReview(packId, conversationId, proposal.id, open);
  const apply = useApplyProposal(packId, conversationId);
  const data = review.data;
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) apply.reset();
        onOpenChange(next);
      }}
    >
      <DialogContent
        size="lg"
        title={t('dialogo.titulo')}
        description={data && !data.changesPack ? t('dialogo.subSemPack') : t('dialogo.sub')}
        footer={
          <>
            <DialogClose asChild>
              <Button variant="ghost">{t('dialogo.cancelar')}</Button>
            </DialogClose>
            <Button
              variant="primary"
              icon={Check}
              disabled={!data}
              loading={apply.isPending}
              onClick={() => {
                if (!data) return;
                apply.mutate(
                  { proposalId: proposal.id, token: data.token },
                  {
                    onSuccess: () => {
                      showToast({ kind: 'ok', title: t('avisos.aplicada') });
                      onOpenChange(false);
                    },
                  },
                );
              }}
            >
              {apply.isPending ? t('dialogo.aplicando') : t('dialogo.aplicar')}
            </Button>
          </>
        }
      >
        <div className="stack-3">
          <p className="ai-apply__title">{proposalTitle(proposal)}</p>
          {apply.isError ? <ErrorPanel compact error={apply.error} /> : null}
          {review.isError ? (
            <ErrorPanel compact title={t('dialogo.erro')} error={review.error} />
          ) : !data ? (
            <LoadingState label={t('dialogo.calculando')} />
          ) : (
            <Preview preview={data.preview} />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function names(list: readonly { name?: string; title?: string }[]): string {
  return list.map((item) => item.name ?? item.title ?? '').join(', ');
}

function Preview({ preview }: { preview: ProposalPreview }) {
  const { t } = useTranslation('iaPropostas');
  switch (preview.kind) {
    case 'addMod': {
      const plan = preview.plan;
      const entering = plan.nodes.filter((node) => node.role !== 'optional');
      const titleOf = (key: string) => plan.nodes.find((node) => node.key === key)?.title ?? key;
      return (
        <div className="stack-2" data-testid="previa-adicionar">
          <p className="t-sm">{t('dialogo.entram', { count: entering.length })}</p>
          <ul className="removelist">
            {entering.map((node) => (
              <li key={node.key}>
                <b>{node.title}</b> {node.versionNumber}
                {node.role === 'required' && node.requiredBy.length > 0 ? (
                  <span className="t-3">
                    {' '}
                    ({t('dialogo.dependencia', { de: node.requiredBy.map(titleOf).join(', ') })})
                  </span>
                ) : null}
              </li>
            ))}
          </ul>
          {plan.installed.length > 0 ? (
            <p className="t-sm t-2">{t('dialogo.jaNoPack', { nomes: names(plan.installed) })}</p>
          ) : null}
          {plan.missing.length > 0 ? (
            <p className="t-sm t-warn">{t('dialogo.semVersao', { nomes: names(plan.missing) })}</p>
          ) : null}
          {plan.conflicts.length > 0 ? (
            <p className="t-sm t-warn">
              {t('dialogo.incompativeis', {
                nomes: plan.conflicts
                  .map((conflict) => `${conflict.item.title} × ${conflict.other.title}`)
                  .join(', '),
              })}
            </p>
          ) : null}
        </div>
      );
    }
    case 'removeMod': {
      const plan = preview.plan;
      return (
        <div className="stack-2" data-testid="previa-remover">
          <p className="t-sm">{t('dialogo.sai', { nomes: names(plan.targets) })}</p>
          {plan.dependents.length > 0 ? (
            <p className="t-sm t-warn">
              {t('dialogo.deixamDeFuncionar', {
                count: plan.dependents.length,
                nomes: names(plan.dependents),
              })}
            </p>
          ) : null}
          {plan.orphans.length > 0 ? (
            <p className="t-sm t-2">{t('dialogo.semUso', { nomes: names(plan.orphans) })}</p>
          ) : null}
        </div>
      );
    }
    case 'changeVersion': {
      const plan = preview.plan;
      return (
        <div className="stack-2" data-testid="previa-versao">
          <p className="t-sm">
            {t('dialogo.versao', { nome: plan.name, de: plan.current ?? '', para: plan.to.number })}
          </p>
          {plan.downgrade ? <p className="t-sm t-warn">{t('dialogo.rebaixamento')}</p> : null}
          {plan.newDependencies.length > 0 ? (
            <p className="t-sm t-warn">
              {t('dialogo.novasDependencias', { nomes: names(plan.newDependencies) })}
            </p>
          ) : null}
          {plan.conflicts.length > 0 ? (
            <p className="t-sm t-warn">
              {t('dialogo.conflitos', {
                nomes: plan.conflicts.map((conflict) => conflict.with).join(', '),
              })}
            </p>
          ) : null}
        </div>
      );
    }
    case 'setSide':
      return (
        <p className="t-sm" data-testid="previa-lado">
          {t('dialogo.lado', {
            nome: preview.name,
            de: sideLabel(preview.from),
            para: sideLabel(preview.to),
          })}
        </p>
      );
    case 'editConfig':
      return <DiffView file={preview.path} before={preview.before} after={preview.after} />;
    case 'testSettings':
      return (
        <div className="stack-2" data-testid="previa-ajustes">
          <p className="t-sm t-2">
            {preview.profile
              ? t('ajustes.perfil', { nome: preview.profile })
              : t('ajustes.perfilPadrao')}
            {' · '}
            {t('ajustes.soNeste')}
          </p>
          <dl className="kv">
            <dt>{t('dialogo.antes')}</dt>
            <dd>{settingsLine(preview.before)}</dd>
            <dt>{t('dialogo.depois')}</dt>
            <dd>{settingsLine(preview.after)}</dd>
          </dl>
        </div>
      );
    case 'runTest':
    case 'startBisect':
      // Abrem o fluxo normal direto do cartão, sem este diálogo.
      return null;
  }
}
