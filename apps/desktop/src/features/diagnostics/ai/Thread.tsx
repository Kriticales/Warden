/**
 * A conversa com o Sculk (SPEC T14 passo 2; protótipo `design/bug-01/sensor-sculk.html`):
 *
 * - "Conversa sobre …" no começo; a sua mensagem num balão à direita, com o **Texto inicial**
 *   recolhido (exatamente o que foi enviado, byte a byte, com Copiar; CA-T14-03);
 * - a resposta do Sculk **sem caixa**, só com o fio à esquerda: o aviso da troca para o modelo
 *   mais leve, o bloco **O Sculk consultou N coisas** (cada consulta "Enviado ao Sculk: …"
 *   aberta mostra o pedido do Sculk e o que foi enviado; as buscas no GitHub da D-14 também), as
 *   afirmações com os selos de evidência (de onde veio o trecho, nunca que a conclusão está
 *   certa; um clique leva à consulta e a destaca), "não verificado", "baseado em mensagem de
 *   jogador" (CA-T14-19), "Encontrar o mod culpado" quando não há conclusão, as propostas e o
 *   rodapé "O Sculk pode errar…" com o medidor calculado pelo Warden;
 * - enquanto o Sculk responde: "Sculk lendo… N s" com as consultas já feitas e a espera visível
 *   (limite por minuto, nova tentativa); um erro diz o que houve, na frase do provedor da
 *   conversa, e nada muda no pack.
 *
 * Sem pack (`packId` nulo, "Analisar um log", D-19): sem links para Mods, sem "Encontrar o mod
 * culpado" e sem propostas.
 */
import { Link } from '@tanstack/react-router';
import {
  ChevronRight,
  CircleAlert,
  CircleHelp,
  Check,
  Clock,
  Copy,
  FileText,
  FlaskConical,
  Info,
  Sparkles,
  User,
} from 'lucide-react';
import { Fragment, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  AiConversationView,
  AiProvider,
  CheckedClaim,
  CheckedEvidence,
  CheckedProposal,
  ConversationTurn,
  KnownMod,
  LiveStatus,
  PackId,
  ToolStep,
} from '../../../lib/ipc/bindings';
import { cn } from '../../../lib/cn';
import { formatBytes, formatTime } from '../../../lib/format';
import { toAppError } from '../../../lib/ipc/errors';
import { log } from '../../../lib/log';
import { aiErrorMessage } from '../../settings/ai/providers';
import { proposalSlots } from './answer-slots';
import { ProposalCards } from './proposals/ProposalCards';
import { turnModel } from './status';
import {
  aboutLabel,
  byteLength,
  evidenceLabel,
  failureError,
  meterBlocks,
  stepLabel,
  symptomLabel,
} from './text';
import { ExternalBlock } from './proposals/ExternalBlock';

/** Id da consulta na página (o selo de evidência e Detalhes levam até ela). */
export function consultDomId(conversationId: string, turn: number, step: number): string {
  return `tc-${conversationId}-${String(turn)}-${String(step)}`;
}

/** Id do Texto inicial na página. */
export function initialDomId(conversationId: string): string {
  return `att-${conversationId}`;
}

const reducedMotion = (): boolean =>
  typeof window !== 'undefined' &&
  typeof window.matchMedia === 'function' &&
  window.matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Rola até o elemento (o jsdom dos testes não tem `scrollIntoView`). */
function scrollTo(element: HTMLElement, block: ScrollLogicalPosition): void {
  if ('scrollIntoView' in element && typeof element.scrollIntoView === 'function') {
    element.scrollIntoView({ block, behavior: reducedMotion() ? 'auto' : 'smooth' });
  }
}

/** Abre uma consulta (e o bloco dela), rola até ela, destaca e põe o foco no resumo. */
export function revealConsult(domId: string): void {
  const item = document.getElementById(domId);
  if (!(item instanceof HTMLDetailsElement)) return;
  const group = item.parentElement?.closest('details');
  if (group instanceof HTMLDetailsElement) group.open = true;
  item.open = true;
  // Reinicia a animação do destaque no quadro seguinte.
  item.classList.remove('is-flash');
  requestAnimationFrame(() => {
    item.classList.add('is-flash');
  });
  scrollTo(item, 'center');
  item.querySelector('summary')?.focus({ preventScroll: true });
}

/** Abre o Texto inicial e leva o foco até ele. */
export function revealInitial(domId: string): void {
  const item = document.getElementById(domId);
  if (!(item instanceof HTMLDetailsElement)) return;
  item.open = true;
  scrollTo(item, 'start');
  item.querySelector('summary')?.focus({ preventScroll: true });
}

/** Copiar para a área de transferência, com o aviso. */
export function useCopy(): (text: string) => void {
  const { t } = useTranslation('ia');
  return (text: string) => {
    navigator.clipboard.writeText(text).then(
      () => {
        showToast({ kind: 'ok', title: t('conversa.copiado') });
      },
      (cause: unknown) => {
        log.warn('falha ao copiar o texto enviado', cause);
        showToast({ kind: 'warn', title: t('conversa.copiarFalhou') });
      },
    );
  };
}

/** Texto exato num bloco rolável que o teclado alcança, com Copiar. */
export function RawBlock({
  text,
  head,
  label,
  short = false,
  wrap = false,
}: {
  text: string;
  head: string;
  label: string;
  short?: boolean;
  wrap?: boolean;
}) {
  const { t } = useTranslation('ia');
  const copy = useCopy();
  return (
    <div>
      <div className={cn('rawhead', !short && 'rawhead--tight')}>
        <span>{head}</span>
        <Button
          size="sm"
          variant="ghost"
          icon={Copy}
          onClick={() => {
            copy(text);
          }}
        >
          {t('conversa.copiar')}
        </Button>
      </div>
      <pre
        className={cn('raw', short && 'raw--short')}
        style={wrap ? { whiteSpace: 'pre-wrap', wordBreak: 'break-word' } : undefined}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
        tabIndex={0}
        aria-label={label}
      >
        {text}
      </pre>
    </div>
  );
}

export interface ThreadProps {
  packId: PackId | null;
  view: AiConversationView;
  /** "Tentar de novo" depois de um erro ou de uma resposta interrompida. */
  onRetry: () => void;
}

export function Thread({ packId, view, onRetry }: ThreadProps) {
  const { t } = useTranslation('ia');
  const id = view.summary.id;
  // Onde está cada evidência citada: a primeira consulta cujo texto enviado traz o id.
  const locate = (evidenceId: string): string | null => {
    for (const [turnIndex, turn] of view.turns.entries()) {
      for (const [stepIndex, step] of turn.steps.entries()) {
        if (step.sent.includes(evidenceId)) return consultDomId(id, turnIndex, stepIndex);
      }
    }
    return null;
  };
  return (
    <div className="thread" data-testid="conversa">
      <div className="thread__start">
        {t('conversa.inicio', { origem: aboutLabel(view.summary.origin) })}
      </div>
      {view.turns.map((turn, index) => (
        <Fragment key={`${String(index)}-${String(turn.askedAtMs)}`}>
          <MeMessage view={view} turn={turn} first={index === 0} />
          <AiMessage
            packId={packId}
            view={view}
            turn={turn}
            index={index}
            last={index === view.turns.length - 1}
            locate={locate}
            onRetry={onRetry}
          />
        </Fragment>
      ))}
    </div>
  );
}

function MeMessage({
  view,
  turn,
  first,
}: {
  view: AiConversationView;
  turn: ConversationTurn;
  first: boolean;
}) {
  const { t } = useTranslation('ia');
  const text = first ? (view.note ?? `${symptomLabel(view.summary.symptom)}.`) : turn.question;
  const bytes = byteLength(turn.question);
  return (
    <article className="msg msg--me" aria-label={t('conversa.suaMensagem')}>
      <div className="msg__head">
        <span>{t('conversa.voce')}</span>
        <span className="msg__meta">{formatTime(turn.askedAtMs)}</span>
      </div>
      <div className="bubble">{text}</div>
      {first ? (
        <details className="att" id={initialDomId(view.summary.id)}>
          <summary>
            <Icon icon={FileText} size="sm" />
            <span>{t('conversa.textoInicial')}</span>
            <span aria-hidden="true">·</span>
            <span className="att__size">{formatBytes(bytes)}</span>
            <Icon icon={ChevronRight} size="sm" className="chev" />
          </summary>
          <div className="att__body">
            <RawBlock
              text={turn.question}
              head={t('conversa.exato', { bytes: formatBytes(bytes) })}
              label={t('conversa.textoInicial')}
            />
          </div>
        </details>
      ) : null}
    </article>
  );
}

function AiMessage({
  packId,
  view,
  turn,
  index,
  last,
  locate,
  onRetry,
}: {
  packId: PackId | null;
  view: AiConversationView;
  turn: ConversationTurn;
  index: number;
  last: boolean;
  locate: (evidenceId: string) => string | null;
  onRetry: () => void;
}) {
  const { t } = useTranslation('ia');
  const live = last ? view.live : null;
  const model = turnModel(turn, view.model);
  const answer = turn.answer;
  const proposals = packId === null ? [] : view.proposals.filter((p) => p.turn === index);
  const manual =
    packId === null || !answer
      ? []
      : answer.checked.proposals.filter((proposal) => proposal.discarded === null);
  return (
    <article className="msg msg--ai" aria-label={t('conversa.mensagemSculk')} aria-busy={!!live}>
      <div className="msg__head">
        <Icon icon={Sparkles} size="sm" />
        <span>{t('conversa.sculk')}</span>
        <span className="msg__meta">
          {answer ? `${formatTime(answer.atMs)} · ${model}` : model}
        </span>
      </div>
      <div className="ai">
        {turn.fallbackModel ? (
          <div className="ai__note">
            <Icon icon={Clock} size="sm" />
            <span>{t(`troca.${turn.fallbackReason ?? 'quota'}`)}</span>
          </div>
        ) : null}
        {!answer && live ? (
          <Reading
            conversationId={view.summary.id}
            turn={turn}
            index={index}
            live={live}
            provider={view.provider}
          />
        ) : turn.steps.length > 0 ? (
          <Consults conversationId={view.summary.id} turn={turn} index={index} />
        ) : null}
        {answer ? (
          <>
            {answer.checked.summary ? (
              <p className="ai__summary">{answer.checked.summary}</p>
            ) : null}
            {answer.roundLimit ? <p className="t-sm t-3">{t('resposta.limite')}</p> : null}
            {answer.checked.claims.length > 0 ? (
              <ul className="claims">
                {answer.checked.claims.map((claim, position) => (
                  <Claim
                    key={`${String(position)}-${claim.statement}`}
                    packId={packId}
                    claim={claim}
                    locate={locate}
                  />
                ))}
              </ul>
            ) : null}
            {answer.checked.noConclusion ? (
              <div className="claim">
                <p>
                  {t('resposta.semConclusao')}{' '}
                  {packId === null
                    ? t('resposta.semConclusaoSemPack')
                    : t('resposta.semConclusaoTexto')}
                </p>
                {packId !== null ? (
                  <div className="claim__ev">
                    <Button size="sm" icon={FlaskConical} asChild>
                      <Link to="/packs/$packId/problemas" params={{ packId }}>
                        {t('resposta.culpado')}
                      </Link>
                    </Button>
                  </div>
                ) : null}
              </div>
            ) : null}
          </>
        ) : null}
        {packId !== null && (proposals.length > 0 || manual.length > 0) ? (
          <div className="props">
            <ProposalCards
              packId={packId}
              conversationId={view.summary.id}
              proposals={proposals}
              busy={view.live !== null}
              origin={view.summary.origin}
              symptom={view.summary.symptom}
            />
            {manual.map((proposal, position) => (
              <ManualProposal
                key={`${String(position)}-${proposal.kind}`}
                packId={packId}
                conversationId={view.summary.id}
                proposal={proposal}
                index={position}
              />
            ))}
          </div>
        ) : null}
        {!answer && !live && turn.failure ? (
          <Failure view={view} turn={turn} onRetry={onRetry} />
        ) : null}
        {!answer && !live && !turn.failure && last ? <Interrupted onRetry={onRetry} /> : null}
        {answer ? (
          <div className="ai__foot">
            <span>
              <Icon icon={Info} size="sm" />
              {t('resposta.podeErrar')}
            </span>
            <Tooltip content={t('confianca.como')}>
              {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a dica do medidor precisa do teclado */}
              <span className="meter" tabIndex={0}>
                {t('confianca.rotulo', { nivel: t(`confianca.${answer.checked.confidence}`) })}
                <span className="meter__blocks" aria-hidden="true">
                  {[1, 2, 3].map((block) => (
                    <i
                      key={block}
                      className={block <= meterBlocks(answer.checked.confidence) ? 'on' : ''}
                    />
                  ))}
                </span>
              </span>
            </Tooltip>
          </div>
        ) : null}
      </div>
    </article>
  );
}

/** Bytes enviados nas consultas de uma pergunta. */
function stepsBytes(steps: readonly ToolStep[]): number {
  return steps.reduce((total, step) => total + byteLength(step.sent), 0);
}

function Consults({
  conversationId,
  turn,
  index,
}: {
  conversationId: string;
  turn: ConversationTurn;
  index: number;
}) {
  const { t } = useTranslation('ia');
  return (
    <details className="consults">
      <summary>
        <Icon icon={Sparkles} size="sm" />
        <span>{t('consultas.titulo', { count: turn.steps.length })}</span>
        <span aria-hidden="true">·</span>
        <span className="mono">
          {t('consultas.enviados', { tamanho: formatBytes(stepsBytes(turn.steps)) })}
        </span>
        <Icon icon={ChevronRight} size="sm" className="chev" />
      </summary>
      <div className="consults__list">
        {turn.steps.map((step, position) => (
          <ConsultItem
            key={`${step.callId ?? ''}-${String(position)}`}
            domId={consultDomId(conversationId, index, position)}
            step={step}
          />
        ))}
      </div>
    </details>
  );
}

function ConsultItem({ domId, step }: { domId: string; step: ToolStep }) {
  const { t } = useTranslation('ia');
  const failed = !step.ok;
  const size = formatBytes(byteLength(step.sent));
  return (
    <details className={cn('tc', failed && 'tc--error')} id={domId}>
      <summary>
        <Icon icon={failed ? CircleAlert : ChevronRight} size="sm" className={failed ? undefined : 'chev'} />
        <span className="tc__title">
          <span className="tc__pre">{t('consultas.pre')}</span> {stepLabel(step)}
          {step.denied
            ? ` · ${t('consultas.negada')}`
            : failed
              ? ` · ${t('consultas.falhou')}`
              : ''}
        </span>
        <span className="tc__size">{size}</span>
      </summary>
      <div className="tc__body">
        {step.external ? <ExternalBlock request={step.external} /> : null}
        <div>
          <div className="tc__label">{t('consultas.pedido')}</div>
          <code className="req">{`${step.name} ${step.args}`}</code>
        </div>
        <RawBlock
          text={step.sent}
          head={t('consultas.oQue', { tamanho: size })}
          label={t('consultas.oQue', { tamanho: size })}
          wrap
        />
      </div>
    </details>
  );
}

/** "Sculk lendo… N s", com as consultas já feitas e a espera visível. */
function Reading({
  conversationId,
  turn,
  index,
  live,
  provider,
}: {
  conversationId: string;
  turn: ConversationTurn;
  index: number;
  live: LiveStatus;
  provider: AiProvider;
}) {
  const { t } = useTranslation('ia');
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, []);
  const seconds = Math.max(0, Math.floor((now - turn.askedAtMs) / 1000));
  let waiting: string | null = null;
  if (live.kind === 'waitingPerMinute') {
    waiting =
      provider === 'gemini'
        ? t('espera.porMinuto', { segundos: live.seconds })
        : t('espera.porMinutoPago', { segundos: live.seconds });
  } else if (live.kind === 'retrying') {
    waiting = t(`espera.tentando.${provider}`, { tentativa: live.attempt, total: live.of });
  }
  return (
    <>
      <div role="status" aria-live="polite">
        <span className="reading">
          <Loader />
          <span>{t('conversa.lendo')}</span>
          <span className="mono" aria-hidden="true">
            {t('conversa.segundos', { segundos: seconds })}
          </span>
        </span>
        {waiting ? <p className="t-sm t-2">{waiting}</p> : null}
        <p className="t-xs t-3">{t('espera.sair')}</p>
      </div>
      {turn.steps.length > 0 ? (
        <div className="consults">
          <div className="consults__list consults__list--run">
            {turn.steps.map((step, position) => (
              <ConsultItem
                key={`${step.callId ?? ''}-${String(position)}`}
                domId={consultDomId(conversationId, index, position)}
                step={step}
              />
            ))}
          </div>
        </div>
      ) : null}
    </>
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

function Claim({
  packId,
  claim,
  locate,
}: {
  packId: PackId | null;
  claim: CheckedClaim;
  locate: (evidenceId: string) => string | null;
}) {
  const { t } = useTranslation('ia');
  const [open, setOpen] = useState<number | null>(null);
  const shown = open === null ? null : claim.evidence[open];
  const unverified = claim.status === 'unverified';
  return (
    <li className={cn('claim', unverified && 'claim--no')}>
      <p>{claim.statement}</p>
      <div className="claim__ev">
        {unverified ? (
          <Tooltip content={t('selo.dicaNaoVerificado')}>
            {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a dica precisa do teclado */}
            <span className="evid evid--no" tabIndex={0}>
              <Icon icon={CircleHelp} size="sm" />
              {t('selo.naoVerificado')}
            </span>
          </Tooltip>
        ) : null}
        {claim.status === 'playerMessage' ? (
          <span className="tag tag--warn">
            <Icon icon={User} />
            {t('selo.jogador')}
          </span>
        ) : null}
        {claim.evidence.map((evidence, position) => (
          <EvidenceBadge
            key={`${evidence.id}-${String(position)}`}
            evidence={evidence}
            open={open === position}
            onOpen={() => {
              const target = locate(evidence.id);
              if (target) {
                setOpen(null);
                revealConsult(target);
              } else {
                setOpen(open === position ? null : position);
              }
            }}
          />
        ))}
        {packId === null
          ? null
          : claim.mods.map((mod) => <ModLink key={mod.path} packId={packId} mod={mod} />)}
      </div>
      {shown?.sourceText ? (
        <div className="ai-excerpt">
          <RawBlock
            text={shown.sourceText}
            head={evidenceLabel(shown.id)}
            label={evidenceLabel(shown.id)}
            wrap
          />
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
  onOpen,
}: {
  evidence: CheckedEvidence;
  open: boolean;
  onOpen: () => void;
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
          onClick={onOpen}
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

/** Proposta do campo "propostas" da resposta (o que nenhuma ferramenta propose_* cobre). */
function ManualProposal({
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
        <span className="proposal__kind">{kind ? t(`proposta.tipos.${kind}`) : proposal.kind}</span>
        {proposal.target ? <span className="proposal__title">{proposal.target.name}</span> : null}
      </div>
      <p className="proposal__text">{proposal.detail}</p>
      <div className="proposal__ev">
        <span>{t('proposta.porque')}</span>
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
        <p className="t-xs t-3">
          <Icon icon={Info} size="sm" /> {t('proposta.manual')}
        </p>
      )}
    </div>
  );
}

function Failure({
  view,
  turn,
  onRetry,
}: {
  view: AiConversationView;
  turn: ConversationTurn;
  onRetry: () => void;
}) {
  const { t } = useTranslation('ia');
  if (!turn.failure) {
    return null;
  }
  const error = failureError(turn.failure);
  const code = turn.failure.code;
  const settingsLink =
    code === 'QUOTA_EXCEEDED' ||
    code === 'KEY_INVALID' ||
    code === 'NOT_CONFIGURED' ||
    code === 'PROVIDER_KEY_MISSING' ||
    code === 'NO_MODEL';
  return (
    <Alert
      kind="danger"
      title={aiErrorMessage(toAppError(error), view.provider)}
      actions={
        <>
          {turn.failure.retryable || code === 'QUOTA_EXCEEDED' ? (
            <Button size="sm" onClick={onRetry}>
              {t('erro.tentar')}
            </Button>
          ) : null}
          {settingsLink ? (
            <Button size="sm" variant="ghost" asChild>
              <Link to="/configuracoes">{t('erro.configuracoes')}</Link>
            </Button>
          ) : null}
        </>
      }
    >
      {t('erro.nadaMudou')}
    </Alert>
  );
}

function Interrupted({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation('ia');
  return (
    <Alert
      kind="warn"
      title={t('interrompida.titulo')}
      actions={
        <Button size="sm" onClick={onRetry}>
          {t('erro.tentar')}
        </Button>
      }
    >
      {t('interrompida.texto')}
    </Alert>
  );
}
