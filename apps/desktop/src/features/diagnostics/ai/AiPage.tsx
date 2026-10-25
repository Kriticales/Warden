/**
 * Seção ✦ Diagnóstico com IA (SPEC T14 ✦ "Na seção"; ESTRUTURA §13; protótipo `ia`): sem a chave
 * do provedor ativo (D-18: Gemini, OpenAI ou Claude), o caminho para Configurações, citando os
 * três provedores, com o Gemini sugerido; com ela, **Nova conversa** (do que se trata, o que
 * analisar, "Contar algo à IA" e **✦ Começar conversa**, que abre o consentimento), o provedor e
 * o modelo em uso. **Conversas** (continuar e apagar) aparece nos dois casos: cada conversa
 * continua no provedor em que começou.
 *
 * Os botões **✦ Pedir ajuda à IA** (resultado do teste) e **✦ Conversar com a IA**
 * (Travamentos) trazem `sessao` na busca: a sessão vem escolhida em "Começar pelo" e o
 * consentimento abre sozinho.
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { KeyRound, Sparkles, Trash2 } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type {
  AiConversationState,
  AiConversationSummary,
  AiLocalPick,
  AiPreview,
  AiProvider,
  AiSessionOption,
  AiSource,
  PackId,
  AiSymptom,
  ModelTier,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { aiErrorMessage, modelFor, providerName } from '../../settings/ai/providers';
import { useSecretsStatus } from '../../settings/api';
import { Choice, StatusText } from '../../settings/components/fields';
import { formatWhen } from '../../test/text';
import {
  useAiConversations,
  useAiLiveUpdates,
  useAiModels,
  useAiPreview,
  useAiSettings,
  useAiSources,
  useChooseLog,
  useDeleteConversation,
} from './api';
import { ConsentDialog } from './ConsentDialog';
import { PendingCount } from './proposals/PendingCount';
import { originLabel, symptomLabel, tokensLabel } from './text';
import './ai.css';

const SYMPTOMS: readonly AiSymptom[] = ['crashOnStart', 'crashInWorld', 'slow', 'other'];
/** Nome do grupo de rádio do sintoma. */
const SYMPTOM_GROUP = 'ia-sintoma';

/** O valor do `<select>` de "Começar pelo". */
type SourceChoice = `sessao:${string}` | 'ultimo' | 'arquivo' | 'pack';
const LAST_CRASH: SourceChoice = 'ultimo';
const LOCAL_FILE: SourceChoice = 'arquivo';
const PACK_ONLY: SourceChoice = 'pack';

export function AiPage({
  packId,
  sessionId,
  subject = null,
  symptom = null,
}: {
  packId: PackId;
  sessionId: string | null;
  /** O assunto vindo de "✦ Perguntar ao Sculk" em Problemas (vai para "Contar algo"). */
  subject?: string | null;
  symptom?: AiSymptom | null;
}) {
  const { t } = useTranslation('ia');
  const secrets = useSecretsStatus();
  const settings = useAiSettings();
  useAiLiveUpdates(packId);

  const head = (
    <PageHead
      title={
        <>
          <Icon icon={Sparkles} size="lg" className="icon--ai" /> {t('secao.titulo')}
        </>
      }
      sub={t('secao.sub', { provedor: providerName(settings.data?.provider ?? 'gemini') })}
    />
  );
  if (secrets.isPending || settings.isPending) {
    return <LoadingState label={t('secao.carregando')} />;
  }
  if (secrets.isError || settings.isError) {
    const error = secrets.error ?? settings.error;
    return (
      <ErrorPanel
        error={error}
        onRetry={() => {
          void secrets.refetch();
          void settings.refetch();
        }}
      />
    );
  }
  const provider = settings.data.provider;
  if (!secrets.data[provider]) {
    return (
      <div className="stack ai-page">
        {head}
        <EmptyState
          glyph="dots"
          title={t(`chave.titulo.${provider}`)}
          text={
            <>
              {t('chave.texto')} {t('chave.provedores')}
            </>
          }
          actions={
            <Link to="/configuracoes" className={buttonVariants({ variant: 'primary' })}>
              <Icon icon={KeyRound} />
              <span>{t('chave.abrir')}</span>
            </Link>
          }
        />
        <Conversations packId={packId} />
      </div>
    );
  }
  return (
    <div className="stack ai-page">
      {head}
      <NewConversation
        packId={packId}
        sessionId={sessionId}
        subject={subject}
        initialSymptom={symptom}
        provider={provider}
      />
      <ModelLine provider={provider} tier={settings.data.tier} />
      <Conversations packId={packId} />
    </div>
  );
}

function NewConversation({
  packId,
  sessionId,
  subject,
  initialSymptom,
  provider,
}: {
  packId: PackId;
  sessionId: string | null;
  subject: string | null;
  initialSymptom: AiSymptom | null;
  provider: AiProvider;
}) {
  const { t } = useTranslation('ia');
  const navigate = useNavigate();
  const sources = useAiSources(packId);
  const preview = useAiPreview(packId);
  const chooseLog = useChooseLog(packId);
  const selectId = useId();
  const noteId = useId();
  const [symptom, setSymptom] = useState<AiSymptom>(initialSymptom ?? 'crashInWorld');
  const [choice, setChoice] = useState<SourceChoice | null>(null);
  const [pick, setPick] = useState<AiLocalPick | null>(null);
  const [note, setNote] = useState(subject ?? '');
  const [consent, setConsent] = useState<AiPreview | null>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const autoOpened = useRef(false);

  const data = sources.data;
  const lastCrash = data?.lastCrash ?? null;
  // Com um assunto e sem sessão (um achado de Problemas), a conversa é sobre o pack.
  const defaultChoice: SourceChoice = sessionId
    ? `sessao:${sessionId}`
    : subject
      ? 'pack'
      : lastCrash
        ? 'ultimo'
        : 'pack';
  const current = choice ?? defaultChoice;

  const sourceOf = (value: SourceChoice): AiSource | null => {
    if (value === 'ultimo') return { kind: 'lastCrash' };
    if (value === 'pack') return { kind: 'packOnly' };
    if (value === 'arquivo') return pick ? { kind: 'localFile', token: pick.token } : null;
    return { kind: 'session', sessionId: value.slice('sessao:'.length) };
  };

  const begin = (value: SourceChoice = current) => {
    const source = sourceOf(value);
    if (!source || preview.isPending) return;
    preview.mutate(
      { symptom, source, note: note.trim() === '' ? null : note },
      {
        onSuccess: (result) => {
          setConsent(result);
        },
      },
    );
  };

  // Vindo de "✦ Pedir ajuda à IA" ou "✦ Conversar com a IA": o consentimento abre sozinho.
  useEffect(() => {
    if ((sessionId || subject) && data && !autoOpened.current) {
      autoOpened.current = true;
      begin(sessionId ? `sessao:${sessionId}` : 'pack');
    }
  });

  const onSelect = (value: SourceChoice) => {
    if (value !== 'arquivo') {
      setChoice(value);
      return;
    }
    chooseLog.mutate(undefined, {
      onSuccess: (picked) => {
        if (picked) {
          setPick(picked);
          setChoice('arquivo');
        }
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  const optionLabel = (option: AiSessionOption, last: boolean): string => {
    const quando = formatWhen(option.startedAtMs);
    return last
      ? t('nova.fonte.ultimo', { quando })
      : t('nova.fonte.sessao', { quando, resultado: t(`resultado.${option.outcome}`) });
  };
  const sessionOptions = (data?.sessions ?? []).filter(
    (option) => option.sessionId !== lastCrash?.sessionId || sessionId === option.sessionId,
  );
  const ready = current !== 'arquivo' || pick !== null;

  return (
    <section className="panel panel--strong" aria-labelledby="ia-nova">
      <h2 className="panel__title panel__title--sans" id="ia-nova">
        {t('nova.titulo')}
      </h2>
      <fieldset className="ai-new__symptoms">
        <legend className="field__label">{t('nova.sintoma')}</legend>
        <div className="choice-list choice-list--2">
          {SYMPTOMS.map((value) => (
            <Choice
              key={value}
              name={SYMPTOM_GROUP}
              title={symptomLabel(value)}
              desc={t(`sintoma.${value}.desc`)}
              checked={symptom === value}
              onSelect={() => {
                setSymptom(value);
              }}
            />
          ))}
        </div>
      </fieldset>
      <div className="grid-2 ai-new__fields">
        <div className="field">
          <label className="field__label" htmlFor={selectId}>
            {t('nova.fonte.rotulo')}
          </label>
          <select
            id={selectId}
            className="select"
            value={current}
            disabled={sources.isPending || chooseLog.isPending}
            onChange={(event) => {
              onSelect(event.target.value as SourceChoice);
            }}
          >
            {lastCrash ? <option value={LAST_CRASH}>{optionLabel(lastCrash, true)}</option> : null}
            {sessionOptions.map((option) => (
              <option key={option.sessionId} value={`sessao:${option.sessionId}`}>
                {optionLabel(option, false)}
              </option>
            ))}
            <option value={LOCAL_FILE}>
              {pick
                ? t('nova.fonte.arquivoEscolhido', { arquivo: pick.fileName })
                : t('nova.fonte.arquivo')}
            </option>
            <option value={PACK_ONLY}>{t('nova.fonte.pack')}</option>
          </select>
          {pick && current === 'arquivo' ? (
            <div className="field__hint">
              {t('nova.fonte.arquivoDica', { tamanho: formatBytes(pick.bytes) })}
            </div>
          ) : null}
        </div>
        <div className="field">
          <label className="field__label" htmlFor={noteId}>
            {t('nova.nota')} <span className="opt">{t('nova.opcional')}</span>
          </label>
          <textarea
            id={noteId}
            className="textarea"
            rows={2}
            maxLength={2000}
            value={note}
            placeholder={t('nova.notaExemplo')}
            onChange={(event) => {
              setNote(event.target.value);
            }}
          />
        </div>
      </div>
      {preview.isError ? (
        <Alert kind="danger" compact title={t('nova.erro')}>
          {aiErrorMessage(toAppError(preview.error), provider)}
        </Alert>
      ) : null}
      <div className="row row--gap-3 ai-new__actions">
        <Button
          ref={opener}
          variant="primary"
          icon={Sparkles}
          loading={preview.isPending}
          disabled={!ready}
          onClick={() => {
            begin();
          }}
        >
          {preview.isPending ? t('nova.preparando') : t('nova.comecar')}
        </Button>
        <span className="t-sm t-3">{t('nova.antes')}</span>
      </div>
      {consent ? (
        <ConsentDialog
          packId={packId}
          preview={consent}
          returnFocusTo={() => opener.current}
          onClose={() => {
            setConsent(null);
          }}
          onStarted={(id) => {
            setConsent(null);
            void navigate({
              to: '/packs/$packId/ia/$conversaId',
              params: { packId, conversaId: id },
            });
          }}
        />
      ) : null}
    </section>
  );
}

/** O provedor e o modelo em uso, com o caminho para trocar em Configurações. */
export function ModelLine({ provider, tier }: { provider: AiProvider; tier: ModelTier }) {
  const { t } = useTranslation('ia');
  const models = useAiModels(provider, true);
  const model = modelFor(models.data, tier);
  let text: string;
  if (models.isPending) {
    text = t('modelo.consultando');
  } else if (models.isError || !model) {
    text = models.isError ? aiErrorMessage(toAppError(models.error), provider) : t('modelo.nenhum');
  } else if (provider === 'gemini') {
    text = t('modelo.atualGemini', { modelo: model });
  } else {
    text = t('modelo.atual', {
      provedor: providerName(provider),
      modelo: model,
      nivel: t(`nivel.${tier}`),
    });
  }
  return (
    <p className="t-sm t-2 ai-model">
      <span>{text}</span>{' '}
      <Link to="/configuracoes" className="link">
        {t('modelo.trocar')}
      </Link>
    </p>
  );
}

const STATE_KIND: Record<AiConversationState, 'ok' | 'warn' | 'danger' | 'muted'> = {
  running: 'muted',
  answered: 'ok',
  failed: 'danger',
  interrupted: 'warn',
};

function Conversations({ packId }: { packId: PackId }) {
  const { t } = useTranslation('ia');
  const list = useAiConversations(packId);
  return (
    <section className="stack-2" aria-labelledby="ia-conversas">
      <h2 className="group-title" id="ia-conversas">
        {t('conversas.titulo')}
      </h2>
      {list.isPending ? (
        <LoadingState label={t('conversas.carregando')} />
      ) : list.isError ? (
        <ErrorPanel
          compact
          error={list.error}
          onRetry={() => {
            void list.refetch();
          }}
        />
      ) : list.data.length === 0 ? (
        <p className="t-sm t-3">{t('conversas.vazio')}</p>
      ) : (
        <div className="tablewrap">
          <table className="table">
            <thead>
              <tr>
                <th>{t('conversas.colunas.conversa')}</th>
                <th>{t('conversas.colunas.quando')}</th>
                <th>{t('conversas.colunas.situacao')}</th>
                <th className="shrink">
                  <span className="sr-only">{t('conversas.colunas.acoes')}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {list.data.map((conversation) => (
                <ConversationRow
                  key={conversation.id}
                  packId={packId}
                  conversation={conversation}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="t-xs t-3">{t('conversas.onde')}</p>
    </section>
  );
}

function ConversationRow({
  packId,
  conversation,
}: {
  packId: PackId;
  conversation: AiConversationSummary;
}) {
  const { t } = useTranslation('ia');
  const remove = useDeleteConversation(packId);
  const title = conversation.title ?? symptomLabel(conversation.symptom);
  return (
    <tr>
      <td>
        <span className="t-strong">{title}</span>
        <div className="t-xs t-3">
          {originLabel(conversation.origin)}
          {conversation.tokens > 0 ? ` · ${tokensLabel(conversation.tokens)}` : ''}
        </div>
      </td>
      <td className="t-2">{formatWhen(conversation.updatedAtMs)}</td>
      <td>
        <StatusText kind={STATE_KIND[conversation.state]}>
          {t(`conversas.situacao.${conversation.state}`)}
        </StatusText>
        <PendingCount count={conversation.pendingProposals} />
      </td>
      <td className="shrink">
        <div className="row">
          <Button size="sm" asChild>
            <Link
              to="/packs/$packId/ia/$conversaId"
              params={{ packId, conversaId: conversation.id }}
              aria-label={t('conversas.continuarRotulo', { titulo: title })}
            >
              {t('conversas.continuar')}
            </Link>
          </Button>
          <ConfirmDialog
            trigger={
              <Button size="sm" variant="ghost" iconOnly icon={Trash2}>
                {t('conversas.apagarRotulo', { titulo: title })}
              </Button>
            }
            title={t('apagar.titulo')}
            description={t('apagar.texto')}
            confirmLabel={t('apagar.confirmar')}
            confirmingLabel={t('apagar.apagando')}
            onConfirm={async () => {
              await remove.mutateAsync(conversation.id);
            }}
          />
        </div>
      </td>
    </tr>
  );
}
