/**
 * Os cartões de proposta de uma resposta da IA (SPEC T14 ✦ passo 3; protótipo `ia-conversa`,
 * `W.proposal`; CA-T14-12): cada mudança que a IA propôs pelas ferramentas `propose_*` (já
 * conferida pelo Warden: proposta sem evidência nem chega aqui), com o título exato, o motivo, a
 * evidência ("Com base em") e **Aplicar** e **Descartar**. Nada muda no pack sem Aplicar.
 *
 * - Aplicar abre o diálogo com a diferença (`ApplyDialog`); testar de novo e a busca do culpado
 *   abrem o fluxo normal (o Testar e o diálogo da busca), e a proposta fica aceita quando o
 *   fluxo começa.
 * - Aplicada: a etiqueta diz quando e se houve ponto de segurança, com **Testar agora** e
 *   **Desfazer** (volta ao ponto de segurança, com confirmação). Descartada: nada foi gravado.
 * - Enquanto a IA responde nesta conversa, os botões esperam (a decisão grava na conversa).
 */
import { Check, Play, Target, Undo2, Wrench } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../../components/common/ConfirmDialog';
import { Button } from '../../../../components/ui/button';
import { Icon } from '../../../../components/ui/icon';
import { showToast } from '../../../../components/ui/toast';
import type { AiSymptom, ConversationOrigin, PackId, Proposal } from '../../../../lib/ipc/bindings';
import { commands } from '../../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../../lib/ipc/errors';
import { unwrap } from '../../../../lib/ipc/result';
import { cn } from '../../../../lib/cn';
import { BisectDialog, type BisectOrigin } from '../../../bisect/BisectDialog';
import { useOpenTest } from '../../../test/TestButton';
import { formatWhen } from '../../../test/text';
import { useGame, useTestStore } from '../../../test/store';
import { useRecoverSafetyPoint } from '../../../versioning/api';
import { evidenceLabel } from '../text';
import { ApplyDialog } from './ApplyDialog';
import { useApplyProposal, useDiscardProposal } from './api';
import { proposalTitle } from './text';
import './proposals.css';

export interface ProposalCardsProps {
  packId: PackId;
  conversationId: string;
  /** As propostas desta resposta. */
  proposals: readonly Proposal[];
  /** A IA está respondendo nesta conversa. */
  busy: boolean;
  /** O que a conversa analisa (o travamento de origem da busca do culpado). */
  origin: ConversationOrigin;
  symptom: AiSymptom;
}

export function ProposalCards({
  packId,
  conversationId,
  proposals,
  busy,
  origin,
  symptom,
}: ProposalCardsProps) {
  const { t } = useTranslation('iaPropostas');
  if (proposals.length === 0) {
    return null;
  }
  return (
    <div className="stack-2" data-testid="propostas">
      <div className="ai-block__label">{t('cartoes.titulo')}</div>
      {proposals.map((proposal) => (
        <ProposalCard
          key={proposal.id}
          packId={packId}
          conversationId={conversationId}
          proposal={proposal}
          busy={busy}
          origin={origin}
          symptom={symptom}
        />
      ))}
      <p className="t-xs t-3">{t('cartoes.nadaMuda')}</p>
    </div>
  );
}

/** O travamento de origem e o sintoma da conversa, para o diálogo da busca do culpado. */
function bisectOrigin(origin: ConversationOrigin, symptom: AiSymptom): BisectOrigin {
  return {
    crash: origin.kind === 'session' ? origin.signature : null,
    symptom: symptom === 'crashInWorld' ? 'crashOnWorld' : 'crashOnOpen',
  };
}

function ProposalCard({
  packId,
  conversationId,
  proposal,
  busy,
  origin,
  symptom,
}: Omit<ProposalCardsProps, 'proposals'> & { proposal: Proposal }) {
  const { t } = useTranslation('iaPropostas');
  const title = proposalTitle(proposal);
  const titleId = `proposta-${conversationId}-${proposal.id}`;
  const state = proposal.state;
  const evidence = proposal.evidence.filter((item) => item.status === 'valid');
  const kind = proposal.change.kind;
  return (
    <section
      className={cn(
        'proposal',
        state.state === 'applied' && 'proposal--applied',
        state.state === 'discarded' && 'proposal--dismissed',
      )}
      aria-labelledby={titleId}
      data-proposal={proposal.id}
    >
      <div className="proposal__head">
        <Icon icon={kind === 'startBisect' ? Target : Wrench} />
        <span className="proposal__kind">{t('cartoes.rotulo')}</span>
        <h3 className="proposal__title" id={titleId}>
          {title}
        </h3>
        {state.state === 'applied' ? (
          <span className="tag tag--ok">
            <Icon icon={Check} />
            {state.safetyPoint
              ? t('estado.aplicadaPonto', { quando: formatWhen(state.atMs) })
              : t('estado.aplicada', { quando: formatWhen(state.atMs) })}
          </span>
        ) : null}
        {state.state === 'discarded' ? <span className="tag">{t('estado.descartada')}</span> : null}
      </div>
      {proposal.reason ? <p className="proposal__text">{proposal.reason}</p> : null}
      {kind === 'startBisect' && state.state === 'pending' ? (
        <p className="proposal__text">{t('busca.texto')}</p>
      ) : null}
      {state.state === 'discarded' ? (
        <p className="proposal__text">{t('estado.descartadaTexto')}</p>
      ) : null}
      {evidence.length > 0 ? (
        <div className="proposal__ev">
          <span className="t-xs t-3">{t('cartoes.comBaseEm')}</span>
          {evidence.map((item, index) => (
            <span key={`${item.id}-${String(index)}`} className="evid">
              <Icon icon={Check} size="sm" />
              {evidenceLabel(item.id)}
            </span>
          ))}
        </div>
      ) : null}
      {state.state === 'pending' ? (
        <PendingActions
          packId={packId}
          conversationId={conversationId}
          proposal={proposal}
          title={title}
          busy={busy}
          origin={origin}
          symptom={symptom}
        />
      ) : null}
      {state.state === 'applied' &&
      proposal.change.kind !== 'runTest' &&
      proposal.change.kind !== 'startBisect' ? (
        <AppliedActions packId={packId} safetyPoint={state.safetyPoint} title={title} />
      ) : null}
    </section>
  );
}

function PendingActions({
  packId,
  conversationId,
  proposal,
  title,
  busy,
  origin,
  symptom,
}: Omit<ProposalCardsProps, 'proposals'> & { proposal: Proposal; title: string }) {
  const { t } = useTranslation('iaPropostas');
  const [applying, setApplying] = useState(false);
  const [bisecting, setBisecting] = useState(false);
  const discard = useDiscardProposal(packId, conversationId);
  const accept = useApplyProposal(packId, conversationId);
  const game = useGame();
  const start = useTestStore((store) => store.start);
  const openTest = useOpenTest();
  const kind = proposal.change.kind;

  /** Testar de novo e a busca do culpado não mudam o pack: aceitar é só gravar a decisão. */
  const acceptDirectly = async () => {
    const review = unwrap(await commands.aiProposalReview(packId, conversationId, proposal.id));
    await accept.mutateAsync({ proposalId: proposal.id, token: review.token });
  };
  const failed = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };

  const main =
    kind === 'runTest' ? (
      <Button
        variant="primary"
        icon={Play}
        disabled={busy || game !== null}
        loading={accept.isPending}
        aria-label={t('acoes.aplicarRotulo', { titulo: title })}
        onClick={() => {
          acceptDirectly()
            .then(() => {
              void start(packId);
              openTest(packId);
            })
            .catch(failed);
        }}
      >
        {t('acoes.testarAgora')}
      </Button>
    ) : kind === 'startBisect' ? (
      <Button
        variant="primary"
        icon={Target}
        disabled={busy || game !== null}
        aria-label={t('acoes.aplicarRotulo', { titulo: title })}
        onClick={() => {
          setBisecting(true);
        }}
      >
        {t('acoes.comecarBusca')}
      </Button>
    ) : (
      <Button
        variant="primary"
        icon={Check}
        disabled={busy}
        aria-label={t('acoes.aplicarRotulo', { titulo: title })}
        onClick={() => {
          setApplying(true);
        }}
      >
        {t('acoes.aplicar')}
      </Button>
    );

  return (
    <>
      <div className="proposal__actions">
        {main}
        <Button
          variant="ghost"
          disabled={busy}
          loading={discard.isPending}
          aria-label={t('acoes.descartarRotulo', { titulo: title })}
          onClick={() => {
            discard.mutate(proposal.id, {
              onSuccess: () => {
                showToast({ kind: 'info', title: t('avisos.descartada') });
              },
              onError: failed,
            });
          }}
        >
          {discard.isPending ? t('acoes.descartando') : t('acoes.descartar')}
        </Button>
      </div>
      {busy ? <p className="t-xs t-3">{t('acoes.esperandoIa')}</p> : null}
      {!busy && game !== null && (kind === 'runTest' || kind === 'startBisect') ? (
        <p className="t-xs t-3">{t('acoes.comJogoAberto')}</p>
      ) : null}
      {kind !== 'runTest' && kind !== 'startBisect' ? (
        <ApplyDialog
          packId={packId}
          conversationId={conversationId}
          proposal={proposal}
          open={applying}
          onOpenChange={setApplying}
        />
      ) : null}
      {kind === 'startBisect' ? (
        <BisectDialog
          packId={packId}
          open={bisecting}
          onOpenChange={setBisecting}
          origin={bisectOrigin(origin, symptom)}
          onStart={() => {
            acceptDirectly().catch(failed);
          }}
        />
      ) : null}
    </>
  );
}

function AppliedActions({
  packId,
  safetyPoint,
  title,
}: {
  packId: PackId;
  safetyPoint: string | null;
  title: string;
}) {
  const { t } = useTranslation('iaPropostas');
  const [undoing, setUndoing] = useState(false);
  const recover = useRecoverSafetyPoint(packId);
  const game = useGame();
  const start = useTestStore((store) => store.start);
  const openTest = useOpenTest();
  return (
    <div className="proposal__actions">
      <Button
        size="sm"
        icon={Play}
        disabled={game !== null}
        onClick={() => {
          void start(packId);
          openTest(packId);
        }}
      >
        {t('acoes.testarAgora')}
      </Button>
      {safetyPoint ? (
        <ConfirmDialog
          open={undoing}
          onOpenChange={setUndoing}
          trigger={
            <Button
              size="sm"
              variant="ghost"
              icon={Undo2}
              aria-label={`${t('acoes.desfazer')}: ${title}`}
            >
              {t('acoes.desfazer')}
            </Button>
          }
          title={t('desfazer.titulo')}
          description={t('desfazer.texto')}
          confirmLabel={t('desfazer.confirmar')}
          confirmingLabel={t('desfazer.confirmando')}
          onConfirm={async () => {
            await recover.mutateAsync(safetyPoint);
            showToast({ kind: 'ok', title: t('avisos.desfeita') });
          }}
        />
      ) : null}
    </div>
  );
}
