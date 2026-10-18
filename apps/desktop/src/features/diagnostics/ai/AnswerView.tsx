/**
 * A resposta da IA conferida pelo Warden (SPEC T14 ✦ passo 2; ADR-0030 e ADR-0055; CA-T14-11 e
 * CA-T14-19; protótipo `ia-conversa`):
 *
 * - cada afirmação com os selos de evidência: o selo diz **de onde veio o trecho** e, aberto,
 *   mostra a linha; nunca que a conclusão está certa (a dica diz isso);
 * - afirmação sem evidência conferida aparece riscada, "não verificado"; a que só se apoia numa
 *   mensagem de chat de um jogador ganha o selo "baseado em mensagem de jogador";
 * - o medidor de confiança é o calculado pelo Warden (a confiança declarada pela IA não entra);
 * - mods citados que existem no pack viram link para a lista de Mods; os inexistentes, não;
 * - "Não sei" é resposta válida: a sugestão é a busca do culpado, em Problemas;
 * - as propostas com evidência aparecem como cartões; os botões delas são da D-14
 *   ([`answerSlots`](./answer-slots.ts)). Proposta sem evidência nem aparece.
 */
import { Link } from '@tanstack/react-router';
import { Check, CircleHelp, Info, Lightbulb, User } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  CheckedClaim,
  CheckedEvidence,
  CheckedProposal,
  FinalAnswer,
  KnownMod,
  PackId,
} from '../../../lib/ipc/bindings';
import { cn } from '../../../lib/cn';
import { proposalSlots } from './answer-slots';
import { ScrollCode } from './ScrollCode';
import { evidenceLabel, meterBlocks } from './text';

export function AnswerView({
  packId,
  conversationId,
  answer,
}: {
  packId: PackId;
  conversationId: string;
  answer: FinalAnswer;
}) {
  const { t } = useTranslation('ia');
  const titleId = useId();
  const checked = answer.checked;
  const proposals = checked.proposals.filter((proposal) => proposal.discarded === null);
  return (
    <section className="ai-answer stack-3" aria-labelledby={titleId}>
      <div>
        <div className="ai-block__label" id={titleId}>
          {t('resposta.titulo')}
        </div>
        <p>{checked.summary}</p>
      </div>
      {answer.roundLimit ? <p className="t-sm t-3">{t('resposta.limite')}</p> : null}
      {checked.claims.length > 0 ? (
        <div>
          <div className="ai-block__label">{t('resposta.afirmacoes')}</div>
          <ul className="claims">
            {checked.claims.map((claim, index) => (
              <Claim key={`${String(index)}-${claim.statement}`} packId={packId} claim={claim} />
            ))}
          </ul>
        </div>
      ) : null}
      {checked.noConclusion ? (
        <Alert
          kind="neutral"
          compact
          title={t('resposta.semConclusao')}
          actions={
            <Button size="sm" asChild>
              <Link to="/packs/$packId/problemas" params={{ packId }}>
                {t('resposta.culpado')}
              </Link>
            </Button>
          }
        >
          {t('resposta.semConclusaoTexto')}
        </Alert>
      ) : null}
      <div className="row row--wrap row--gap-4">
        <div>
          <div className="ai-block__label">{t('confianca.titulo')}</div>
          <span className="meter">
            <span className="meter__blocks" aria-hidden="true">
              {[1, 2, 3].map((block) => (
                <i key={block} className={block <= meterBlocks(checked.confidence) ? 'on' : ''} />
              ))}
            </span>
            <span>{t(`confianca.${checked.confidence}`)}</span>
          </span>
          <div className="t-xs t-3">{t('confianca.como')}</div>
        </div>
        {checked.mods.length > 0 ? (
          <div>
            <div className="ai-block__label">{t('resposta.mods')}</div>
            <div className="row row--wrap">
              {checked.mods.map((mod) => (
                <ModLink key={mod.path} packId={packId} mod={mod} />
              ))}
            </div>
          </div>
        ) : null}
      </div>
      <Alert kind="neutral" compact title={t('resposta.podeErrar')}>
        {t('resposta.podeErrarTexto')}
      </Alert>
      {proposals.length > 0 ? (
        <div className="stack-2">
          <div className="ai-block__label">{t('proposta.titulo')}</div>
          {proposals.map((proposal, index) => (
            <Proposal
              key={`${String(index)}-${proposal.kind}`}
              packId={packId}
              conversationId={conversationId}
              proposal={proposal}
              index={index}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

function ModLink({ packId, mod }: { packId: PackId; mod: KnownMod }) {
  return (
    <Button size="sm" variant="link" asChild>
      <Link to="/packs/$packId/mods" params={{ packId }} search={{ item: mod.path }}>
        {mod.name}
      </Link>
    </Button>
  );
}

function Claim({ packId, claim }: { packId: PackId; claim: CheckedClaim }) {
  const { t } = useTranslation('ia');
  const [open, setOpen] = useState<number | null>(null);
  const shown = open === null ? null : claim.evidence[open];
  const unverified = claim.status === 'unverified';
  return (
    <li className={cn('claim', unverified && 'claim--no')}>
      <p>{claim.statement}</p>
      <div className="claim__ev">
        {claim.status === 'unverified' ? (
          <span className="tag">
            <Icon icon={CircleHelp} />
            {t('selo.naoVerificado')}
          </span>
        ) : null}
        {claim.status === 'playerMessage' ? (
          <span className="tag tag--warn">
            <Icon icon={User} />
            {t('selo.jogador')}
          </span>
        ) : null}
        {claim.evidence.map((evidence, index) => (
          <EvidenceBadge
            key={`${evidence.id}-${String(index)}`}
            evidence={evidence}
            open={open === index}
            onToggle={() => {
              setOpen(open === index ? null : index);
            }}
          />
        ))}
        {claim.mods.map((mod) => (
          <ModLink key={mod.path} packId={packId} mod={mod} />
        ))}
      </div>
      {shown?.sourceText ? (
        <div className="ai-excerpt">
          <div className="t-caps t-3">{t('selo.trechoDe', { onde: evidenceLabel(shown.id) })}</div>
          <ScrollCode label={t('selo.trechoDe', { onde: evidenceLabel(shown.id) })}>
            {shown.sourceText}
          </ScrollCode>
        </div>
      ) : null}
      {claim.unknownMods.length > 0 ? (
        <p className="t-xs t-3">
          {t('selo.modsInexistentes', { mods: claim.unknownMods.join(', ') })}
        </p>
      ) : null}
    </li>
  );
}

function EvidenceBadge({
  evidence,
  open,
  onToggle,
}: {
  evidence: CheckedEvidence;
  open: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation('ia');
  const onde = evidenceLabel(evidence.id);
  if (evidence.status === 'valid' || evidence.status === 'playerMessage') {
    const player = evidence.status === 'playerMessage';
    return (
      <Tooltip content={player ? t('selo.dicaJogador', { onde }) : t('selo.dica', { onde })}>
        <button
          type="button"
          className={cn('evid', player && 'evid--player')}
          aria-expanded={open}
          onClick={onToggle}
        >
          <Icon icon={player ? User : Check} size="sm" />
          <span className="sr-only">
            {player ? t('selo.jogadorRotulo') : t('selo.conferidoRotulo')}
          </span>{' '}
          {onde}
        </button>
      </Tooltip>
    );
  }
  return (
    <span className="evid evid--no">
      <Icon icon={CircleHelp} size="sm" />
      {evidence.status === 'unknownId' ? t('selo.idDesconhecido') : t('selo.naoVerificado')}
    </span>
  );
}

const KINDS = [
  'atualizar_mod',
  'remover_mod',
  'voltar_versao_mod',
  'mudar_lado',
  'editar_config',
  'ajustar_teste',
  'testar_de_novo',
  'buscar_culpado',
] as const;

function kindKey(kind: string): (typeof KINDS)[number] | null {
  return KINDS.find((known) => known === kind) ?? null;
}

function Proposal({
  packId,
  conversationId,
  proposal,
  index,
}: {
  packId: PackId;
  conversationId: string;
  proposal: CheckedProposal;
  index: number;
}) {
  const { t } = useTranslation('ia');
  const kind = kindKey(proposal.kind);
  return (
    <div className="proposal">
      <div className="proposal__head">
        <Icon icon={Lightbulb} />
        <span className="proposal__kind">{kind ? t(`proposta.tipos.${kind}`) : proposal.kind}</span>
        {proposal.target ? <span className="proposal__title">{proposal.target.name}</span> : null}
      </div>
      <p className="proposal__text">{proposal.detail}</p>
      <div className="proposal__ev">
        <span className="t-xs t-3">{t('proposta.porque')}</span>
        {proposal.evidence
          .filter((evidence) => evidence.status === 'valid')
          .map((evidence, position) => (
            <span key={`${evidence.id}-${String(position)}`} className="evid">
              <Icon icon={Check} size="sm" />
              {evidenceLabel(evidence.id)}
            </span>
          ))}
      </div>
      {proposalSlots.length > 0 ? (
        <div className="proposal__actions">
          {proposalSlots.map((Slot, position) => (
            <Slot
              key={position}
              packId={packId}
              conversationId={conversationId}
              proposal={proposal}
              index={index}
            />
          ))}
        </div>
      ) : (
        <p className="t-xs t-3 ai-proposal__hint">
          <Icon icon={Info} size="sm" /> {t('proposta.manual')}
        </p>
      )}
    </div>
  );
}
