/**
 * A conversa sem pack (ROADMAP D-19; SPEC T14 item 8), com "← Analisar um log": os mesmos blocos
 * da conversa com pack (o texto inicial e cada consulta como **Enviado à IA: …**, byte a byte; as
 * buscas no GitHub como **Enviado ao GitHub: …**; a resposta conferida com os selos de evidência;
 * a espera visível; o erro com "Tentar de novo"), sem nada que dependa de um pack: sem aviso de
 * pack mudado, sem propostas e sem "Encontrar o mod culpado".
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { Send, Sparkles, Trash2, User } from 'lucide-react';
import { Fragment, useId, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { AiConversationView, ConversationTurn } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { formatWhen } from '../../test/text';
import { AnswerView } from '../ai/AnswerView';
import { Waiting } from '../ai/ConversationPage';
import { ExternalBlock } from '../ai/proposals/ExternalBlock';
import { SentBlock, StepBlock } from '../ai/StepBlock';
import { failureError, originLabel, symptomLabel, tokensLabel } from '../ai/text';
import {
  useDeleteStandalone,
  useRetryStandalone,
  useSendStandalone,
  useStandaloneConversation,
  useStandaloneLiveUpdates,
} from './api';
import '../ai/ai.css';

export function StandaloneConversation({ conversationId }: { conversationId: string }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const conversation = useStandaloneConversation(conversationId);
  useStandaloneLiveUpdates();
  let body;
  if (conversation.isPending) {
    body = <LoadingState label={ti('conversa.carregando')} />;
  } else if (conversation.isError) {
    body = (
      <>
        <PageHead title={ti('conversa.naoAchou')} sans />
        <ErrorPanel error={conversation.error} />
      </>
    );
  } else {
    const view = conversation.data;
    const title = view.summary.title ?? symptomLabel(view.summary.symptom);
    body = (
      <>
        <PageHead
          title={title}
          sub={ti('conversa.sub', {
            origem: originLabel(view.summary.origin),
            quando: formatWhen(view.summary.createdAtMs),
          })}
          sans
        />
        <div className="chatpage">
          <div className="stack">
            <div className="chat">
              {view.turns.map((turn, index) => (
                <Turn
                  key={`${String(index)}-${String(turn.askedAtMs)}`}
                  view={view}
                  turn={turn}
                  first={index === 0}
                  last={index === view.turns.length - 1}
                />
              ))}
            </div>
            <Ask view={view} />
          </div>
          <Side view={view} />
        </div>
      </>
    );
  }
  return (
    <AppPage back={{ to: '/analisar-log', label: t('pagina.titulo') }} where={t('pagina.titulo')}>
      <div className="stack">{body}</div>
    </AppPage>
  );
}

function Turn({
  view,
  turn,
  first,
  last,
}: {
  view: AiConversationView;
  turn: ConversationTurn;
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
            <AnswerView packId={null} conversationId={view.summary.id} answer={turn.answer} />
          ) : null}
          {!turn.answer && live ? <Waiting live={live} /> : null}
          {!turn.answer && !live && turn.failure ? <Failure view={view} turn={turn} /> : null}
          {!turn.answer && !live && !turn.failure && last ? (
            <Interrupted id={view.summary.id} />
          ) : null}
        </div>
      </article>
    </>
  );
}

function Failure({ view, turn }: { view: AiConversationView; turn: ConversationTurn }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const retry = useRetryStandalone(view.summary.id);
  if (!turn.failure) {
    return null;
  }
  const code = turn.failure.code;
  const settingsLink =
    code === 'QUOTA_EXCEEDED' ||
    code === 'KEY_INVALID' ||
    code === 'NOT_CONFIGURED' ||
    code === 'NO_MODEL' ||
    code === 'AI_PROVIDER_KEY_MISSING';
  return (
    <Alert
      kind="danger"
      title={appErrorMessage(failureError(turn.failure))}
      actions={
        <>
          {turn.failure.retryable || code === 'QUOTA_EXCEEDED' ? (
            <Button size="sm" onClick={retry}>
              {ti('erro.tentar')}
            </Button>
          ) : null}
          {settingsLink ? (
            <Button size="sm" variant="ghost" asChild>
              <Link to="/configuracoes">{ti('erro.configuracoes')}</Link>
            </Button>
          ) : null}
        </>
      }
    >
      {t('erro.guardada')}
    </Alert>
  );
}

function Interrupted({ id }: { id: string }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const retry = useRetryStandalone(id);
  return (
    <Alert
      kind="warn"
      title={ti('interrompida.titulo')}
      actions={
        <Button size="sm" onClick={retry}>
          {ti('erro.tentar')}
        </Button>
      }
    >
      {t('erro.interrompida')}
    </Alert>
  );
}

/** "Perguntar mais à IA": só depois da resposta (ou do erro tratado). */
function Ask({ view }: { view: AiConversationView }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const id = useId();
  const [text, setText] = useState('');
  const send = useSendStandalone(view.summary.id);
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
          {ti('perguntar.rotulo')}
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
          {waiting && !busy ? ti('perguntar.depoisDoErro') : ti('perguntar.dica')}
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
          {ti('perguntar.enviar')}
        </Button>
      </div>
    </form>
  );
}

function Side({ view }: { view: AiConversationView }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const navigate = useNavigate();
  const remove = useDeleteStandalone();
  const reads = [t('lado.log'), ...(view.permissions.github ? [t('lado.github')] : [])].join(', ');
  return (
    <aside className="chatpage__side" aria-label={ti('lado.rotulo')}>
      <div className="panel">
        <dl className="kv">
          <dt>{ti('lado.comecou')}</dt>
          <dd>{formatWhen(view.summary.createdAtMs)}</dd>
          <dt>{ti('lado.modelo')}</dt>
          <dd>{view.model}</dd>
          <dt>{ti('lado.tamanho')}</dt>
          <dd>{tokensLabel(view.summary.tokens)}</dd>
          {view.initialTokens !== null ? (
            <>
              <dt>{ti('lado.inicial')}</dt>
              <dd>{ti('lado.inicialValor', { tokens: tokensLabel(view.initialTokens) })}</dd>
            </>
          ) : null}
          <dt>{ti('lado.requisicoes')}</dt>
          <dd>{view.requests}</dd>
          <dt>{ti('lado.podeLer')}</dt>
          <dd>{reads}</dd>
        </dl>
      </div>
      <p className="t-xs t-3">{ti('lado.tudo')}</p>
      <ConfirmDialog
        trigger={
          <Button size="sm" variant="danger-ghost" icon={Trash2}>
            {ti('lado.apagar')}
          </Button>
        }
        title={ti('apagar.titulo')}
        description={t('apagar.texto')}
        confirmLabel={ti('apagar.confirmar')}
        confirmingLabel={ti('apagar.apagando')}
        onConfirm={async () => {
          await remove.mutateAsync(view.summary.id);
          void navigate({ to: '/analisar-log' });
        }}
      />
    </aside>
  );
}
