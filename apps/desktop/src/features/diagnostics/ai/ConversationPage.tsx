/**
 * A conversa (SPEC T14 ✦ passo 2; protótipo `ia-conversa`), com "← Conversas":
 *
 * - cada consulta da IA vira um bloco recolhido **Enviado à IA: …**, que mostra o que ela pediu e
 *   byte a byte o que saiu (CA-T14-03); o texto inicial também;
 * - a resposta traz as afirmações com os selos de evidência (de onde veio o trecho, nunca que a
 *   conclusão está certa), "não verificado", "baseado em mensagem de jogador" (CA-T14-19), o
 *   medidor calculado pelo Warden e o aviso fixo "A IA pode errar";
 * - enquanto a IA responde: a espera visível (limite por minuto do plano gratuito, nova
 *   tentativa); um erro diz o que houve, nada muda no pack e a conversa fica guardada;
 * - continuar uma conversa antiga avisa se o pack mudou (CA-T14-15);
 * - D-14: as buscas no GitHub aparecem como **Enviado ao GitHub: …** antes do bloco da consulta,
 *   e as propostas da IA como cartões com Aplicar e Descartar depois da resposta
 *   (`proposals/`).
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { ArrowLeft, Send, Sparkles, Trash2, User } from 'lucide-react';
import { Fragment, useId, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import type {
  AiConversationView,
  ConversationTurn,
  LiveStatus,
  PackChanges,
  PackId,
} from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { formatWhen } from '../../test/text';
import { AnswerView } from './AnswerView';
import { ExternalBlock } from './proposals/ExternalBlock';
import { ProposalCards } from './proposals/ProposalCards';
import {
  useAiConversation,
  useAiLiveUpdates,
  useDeleteConversation,
  useRetryConversation,
  useSendQuestion,
} from './api';
import { SentBlock, StepBlock } from './StepBlock';
import { failureError, originLabel, symptomLabel, tokensLabel } from './text';
import './ai.css';

export function ConversationPage({
  packId,
  conversationId,
}: {
  packId: PackId;
  conversationId: string;
}) {
  const { t } = useTranslation('ia');
  const conversation = useAiConversation(packId, conversationId);
  useAiLiveUpdates(packId);
  const back = (
    <Link
      to="/packs/$packId/ia"
      params={{ packId }}
      className={buttonVariants({ variant: 'ghost', size: 'sm' })}
    >
      <Icon icon={ArrowLeft} />
      <span>{t('conversa.voltar')}</span>
    </Link>
  );
  if (conversation.isPending) {
    return <LoadingState label={t('conversa.carregando')} />;
  }
  if (conversation.isError) {
    return (
      <div className="stack">
        <PageHead title={t('conversa.naoAchou')} back={back} sans />
        <ErrorPanel error={conversation.error} />
      </div>
    );
  }
  const view = conversation.data;
  const title = view.summary.title ?? symptomLabel(view.summary.symptom);
  return (
    <div className="stack">
      <PageHead
        title={title}
        sub={t('conversa.sub', {
          origem: originLabel(view.summary.origin),
          quando: formatWhen(view.summary.createdAtMs),
        })}
        back={back}
        sans
      />
      <div className="chatpage">
        <div className="stack">
          <ChangedAlert
            packId={packId}
            changes={view.packChanges}
            since={view.summary.createdAtMs}
          />
          <div className="chat">
            {view.turns.map((turn, index) => (
              <Turn
                key={`${String(index)}-${String(turn.askedAtMs)}`}
                packId={packId}
                view={view}
                turn={turn}
                index={index}
                first={index === 0}
                last={index === view.turns.length - 1}
              />
            ))}
          </div>
          <Ask packId={packId} view={view} />
        </div>
        <Side packId={packId} view={view} />
      </div>
    </div>
  );
}

function ChangedAlert({
  packId,
  changes,
  since,
}: {
  packId: PackId;
  changes: PackChanges;
  since: number;
}) {
  const { t } = useTranslation('ia');
  const [dismissed, setDismissed] = useState(false);
  const any = changes.updated + changes.added + changes.removed > 0 || changes.other;
  if (!any || dismissed) {
    return null;
  }
  const parts: string[] = [];
  if (changes.updated > 0) parts.push(t('mudou.atualizados', { count: changes.updated }));
  if (changes.added > 0) parts.push(t('mudou.adicionados', { count: changes.added }));
  if (changes.removed > 0) parts.push(t('mudou.removidos', { count: changes.removed }));
  const quando = formatWhen(since);
  return (
    <Alert
      kind="warn"
      title={t('mudou.titulo')}
      actions={
        <>
          <Button size="sm" icon={Sparkles} asChild>
            <Link to="/packs/$packId/ia" params={{ packId }}>
              {t('mudou.nova')}
            </Link>
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setDismissed(true);
            }}
          >
            {t('mudou.continuar')}
          </Button>
        </>
      }
    >
      {parts.length > 0
        ? t('mudou.itens', { mudancas: parts.join(', '), quando })
        : t('mudou.outros', { quando })}
    </Alert>
  );
}

function Turn({
  packId,
  view,
  turn,
  index,
  first,
  last,
}: {
  packId: PackId;
  view: AiConversationView;
  turn: ConversationTurn;
  index: number;
  first: boolean;
  last: boolean;
}) {
  const { t } = useTranslation('ia');
  const live = last ? view.live : null;
  return (
    <>
      {first ? (
        <SentBlock title={t('conversa.textoInicial')} text={turn.question} />
      ) : (
        <article className="msg msg--user" aria-label={t('conversa.suaMensagem')}>
          <div className="msg__head">
            <Icon icon={User} />
            <span>{t('conversa.voce')}</span>
            <span className="msg__meta">{formatWhen(turn.askedAtMs)}</span>
          </div>
          <div className="msg__body">{turn.question}</div>
        </article>
      )}
      <article className="msg msg--ai" aria-label={t('conversa.mensagemIa')}>
        <div className="msg__head">
          <Icon icon={Sparkles} className="icon--ai" />
          <span>{t('conversa.ia')}</span>
          <span className="msg__meta">
            {turn.answer ? `${view.model} · ${formatWhen(turn.answer.atMs)}` : view.model}
          </span>
        </div>
        <div className="msg__body">
          {turn.steps.map((step, position) => (
            <Fragment key={`${step.callId ?? ''}-${String(position)}`}>
              {step.external ? <ExternalBlock request={step.external} /> : null}
              <StepBlock step={step} />
            </Fragment>
          ))}
          {turn.answer ? (
            <AnswerView packId={packId} conversationId={view.summary.id} answer={turn.answer} />
          ) : null}
          <ProposalCards
            packId={packId}
            conversationId={view.summary.id}
            proposals={view.proposals.filter((proposal) => proposal.turn === index)}
            busy={view.live !== null}
            origin={view.summary.origin}
            symptom={view.summary.symptom}
          />
          {!turn.answer && live ? <Waiting live={live} /> : null}
          {!turn.answer && !live && turn.failure ? (
            <Failure packId={packId} view={view} turn={turn} />
          ) : null}
          {!turn.answer && !live && !turn.failure && last ? (
            <Interrupted packId={packId} id={view.summary.id} />
          ) : null}
        </div>
      </article>
    </>
  );
}

export function Waiting({ live }: { live: LiveStatus }) {
  const { t } = useTranslation('ia');
  let text: string;
  if (live.kind === 'waitingPerMinute') {
    text = t('espera.porMinuto', { segundos: live.seconds });
  } else if (live.kind === 'retrying') {
    text = t('espera.tentando', { tentativa: live.attempt, total: live.of });
  } else {
    text = t('espera.lendo');
  }
  return (
    <div className="row t-sm t-2" role="status" aria-live="polite">
      <Loader />
      <span>{text}</span>
    </div>
  );
}

function Failure({
  packId,
  view,
  turn,
}: {
  packId: PackId;
  view: AiConversationView;
  turn: ConversationTurn;
}) {
  const { t } = useTranslation('ia');
  const { retry } = useRetryConversation(packId, view.summary.id);
  if (!turn.failure) {
    return null;
  }
  const error = failureError(turn.failure);
  const code = turn.failure.code;
  const settingsLink =
    code === 'QUOTA_EXCEEDED' ||
    code === 'KEY_INVALID' ||
    code === 'NOT_CONFIGURED' ||
    code === 'NO_MODEL';
  return (
    <Alert
      kind="danger"
      title={appErrorMessage(error)}
      actions={
        <>
          {turn.failure.retryable || code === 'QUOTA_EXCEEDED' ? (
            <Button size="sm" onClick={retry}>
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

function Interrupted({ packId, id }: { packId: PackId; id: string }) {
  const { t } = useTranslation('ia');
  const { retry } = useRetryConversation(packId, id);
  return (
    <Alert
      kind="warn"
      title={t('interrompida.titulo')}
      actions={
        <Button size="sm" onClick={retry}>
          {t('erro.tentar')}
        </Button>
      }
    >
      {t('interrompida.texto')}
    </Alert>
  );
}

/** "Perguntar mais à IA": só depois da resposta (ou do erro tratado). */
function Ask({ packId, view }: { packId: PackId; view: AiConversationView }) {
  const { t } = useTranslation('ia');
  const id = useId();
  const [text, setText] = useState('');
  const send = useSendQuestion(packId, view.summary.id);
  const lastTurn = view.turns.at(-1);
  const busy = view.live !== null || send.isPending;
  const waiting = !lastTurn?.answer;
  const disabled = busy || waiting;
  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    if (text.trim() === '' || disabled) return;
    send.mutate(text, {
      onSuccess: () => {
        setText('');
      },
    });
  };
  return (
    <form className="panel ai-ask" onSubmit={submit} aria-labelledby={`${id}-label`}>
      <div className="field">
        <label className="field__label" id={`${id}-label`} htmlFor={id}>
          {t('perguntar.rotulo')}
        </label>
        <textarea
          id={id}
          className="textarea"
          rows={2}
          maxLength={8000}
          value={text}
          disabled={disabled}
          placeholder={t('perguntar.exemplo')}
          aria-describedby={`${id}-hint`}
          onChange={(event) => {
            setText(event.target.value);
          }}
        />
        <div className="field__hint" id={`${id}-hint`}>
          {waiting && !busy ? t('perguntar.depoisDoErro') : t('perguntar.dica')}
        </div>
      </div>
      {send.isError ? (
        <Alert kind="danger" compact>
          {appErrorMessage(toAppError(send.error))}
        </Alert>
      ) : null}
      <div className="row">
        <Button
          type="submit"
          variant="primary"
          icon={Send}
          disabled={disabled || text.trim() === ''}
          loading={send.isPending}
        >
          {t('perguntar.enviar')}
        </Button>
      </div>
    </form>
  );
}

function Side({ packId, view }: { packId: PackId; view: AiConversationView }) {
  const { t } = useTranslation('ia');
  const navigate = useNavigate();
  const remove = useDeleteConversation(packId);
  const reads = [
    t('lado.leituras.pack'),
    t('lado.leituras.logs'),
    ...(view.permissions.configs ? [t('lado.leituras.configs')] : []),
  ].join(', ');
  return (
    <aside className="chatpage__side" aria-label={t('lado.rotulo')}>
      <div className="panel">
        <dl className="kv">
          <dt>{t('lado.comecou')}</dt>
          <dd>{formatWhen(view.summary.createdAtMs)}</dd>
          <dt>{t('lado.modelo')}</dt>
          <dd>{view.model}</dd>
          <dt>{t('lado.tamanho')}</dt>
          <dd>{tokensLabel(view.summary.tokens)}</dd>
          {view.initialTokens !== null ? (
            <>
              <dt>{t('lado.inicial')}</dt>
              <dd>{t('lado.inicialValor', { tokens: tokensLabel(view.initialTokens) })}</dd>
            </>
          ) : null}
          <dt>{t('lado.requisicoes')}</dt>
          <dd>{view.requests}</dd>
          <dt>{t('lado.podeLer')}</dt>
          <dd>{reads}</dd>
        </dl>
      </div>
      <p className="t-xs t-3">{t('lado.tudo')}</p>
      <ConfirmDialog
        trigger={
          <Button size="sm" variant="danger-ghost" icon={Trash2}>
            {t('lado.apagar')}
          </Button>
        }
        title={t('apagar.titulo')}
        description={t('apagar.texto')}
        confirmLabel={t('apagar.confirmar')}
        confirmingLabel={t('apagar.apagando')}
        onConfirm={async () => {
          await remove.mutateAsync(view.summary.id);
          void navigate({ to: '/packs/$packId/ia', params: { packId } });
        }}
      />
    </aside>
  );
}
