/**
 * Seção ✦ Diagnóstico com IA (SPEC T14 ✦ "Na seção"; ESTRUTURA §13; protótipo `ia`): sem a chave
 * do Gemini, o caminho para Configurações; com ela, **Nova conversa** (do que se trata, o que
 * analisar, "Contar algo à IA" e **✦ Começar conversa**, que abre o consentimento), o modelo em
 * uso e **Conversas** (continuar e apagar).
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
  AiSessionOption,
  AiSource,
  PackId,
  Symptom,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
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
import { originLabel, symptomLabel, tokensLabel } from './text';
import './ai.css';

const SYMPTOMS: readonly Symptom[] = ['crashOnStart', 'crashInWorld', 'slow', 'other'];
/** Nome do grupo de rádio do sintoma. */
const SYMPTOM_GROUP = 'ia-sintoma';

/** O valor do `<select>` de "Começar pelo". */
type SourceChoice = `sessao:${string}` | 'ultimo' | 'arquivo' | 'pack';
const LAST_CRASH: SourceChoice = 'ultimo';
const LOCAL_FILE: SourceChoice = 'arquivo';
const PACK_ONLY: SourceChoice = 'pack';

export function AiPage({ packId, sessionId }: { packId: PackId; sessionId: string | null }) {
  const { t } = useTranslation('ia');
  const secrets = useSecretsStatus();
  useAiLiveUpdates(packId);

  const head = (
    <PageHead
      title={
        <>
          <Icon icon={Sparkles} size="lg" className="icon--ai" /> {t('secao.titulo')}
        </>
      }
      sub={t('secao.sub')}
    />
  );
  if (secrets.isPending) {
    return <LoadingState label={t('secao.carregando')} />;
  }
  if (secrets.isError) {
    return <ErrorPanel error={secrets.error} />;
  }
  if (!secrets.data.gemini) {
    return (
      <div className="stack ai-page">
        {head}
        <EmptyState
          glyph="dots"
          title={t('chave.titulo')}
          text={t('chave.texto')}
          actions={
            <Link to="/configuracoes" className={buttonVariants({ variant: 'primary' })}>
              <Icon icon={KeyRound} />
              <span>{t('chave.abrir')}</span>
            </Link>
          }
        />
      </div>
    );
  }
  return (
    <div className="stack ai-page">
      {head}
      <NewConversation packId={packId} sessionId={sessionId} />
      <ModelLine />
      <Conversations packId={packId} />
    </div>
  );
}

function NewConversation({ packId, sessionId }: { packId: PackId; sessionId: string | null }) {
  const { t } = useTranslation('ia');
  const navigate = useNavigate();
  const sources = useAiSources(packId);
  const preview = useAiPreview(packId);
  const chooseLog = useChooseLog(packId);
  const selectId = useId();
  const noteId = useId();
  const [symptom, setSymptom] = useState<Symptom>('crashInWorld');
  const [choice, setChoice] = useState<SourceChoice | null>(null);
  const [pick, setPick] = useState<AiLocalPick | null>(null);
  const [note, setNote] = useState('');
  const [consent, setConsent] = useState<AiPreview | null>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const autoOpened = useRef(false);

  const data = sources.data;
  const lastCrash = data?.lastCrash ?? null;
  const defaultChoice: SourceChoice = sessionId
    ? `sessao:${sessionId}`
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
    if (sessionId && data && !autoOpened.current) {
      autoOpened.current = true;
      begin(`sessao:${sessionId}`);
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
          {appErrorMessage(toAppError(preview.error))}
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

/** O modelo em uso, com o caminho para trocar em Configurações. */
function ModelLine() {
  const { t } = useTranslation('ia');
  const settings = useAiSettings();
  const models = useAiModels(true);
  if (!settings.data) {
    return null;
  }
  const tier = settings.data.tier;
  const model =
    tier === 'careful'
      ? (models.data?.careful ?? models.data?.economical)
      : (models.data?.economical ?? models.data?.careful);
  let text: string;
  if (models.isPending) {
    text = t('modelo.consultando');
  } else if (models.isError || !model) {
    text = models.isError ? appErrorMessage(toAppError(models.error)) : t('modelo.nenhum');
  } else {
    text = t('modelo.atual', { modelo: model, nivel: t(`nivel.${tier}`) });
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
