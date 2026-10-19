/**
 * ✦ Analisar um log (ROADMAP D-19; SPEC T14 item 8): a conversa com a IA **sem pack**, aberta pelo
 * botão de Meus packs. Página do nível do app (como Configurações), porque a ESTRUTURA não tem
 * lugar para isso dentro de um pack: não há pack.
 *
 * - **Nova análise:** o sintoma (o mesmo seletor da conversa com pack), a origem (**um link
 *   colado**, do mclo.gs ou do pastebin, ou **um arquivo do computador**), "Contar algo à IA" e
 *   **✦ Começar conversa**, que baixa o texto do link (só ele vai à rede), tira os dados pessoais
 *   e abre o consentimento. Um link de outro site mostra a frase de recusa sem rede.
 * - O modelo em uso, com o caminho para Configurações (o mesmo da conversa com pack).
 * - **Conversas sem pack:** continuar e apagar.
 *
 * Sem a chave do provedor de IA escolhido, o caminho para Configurações, como na seção do pack.
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { FileText, KeyRound, Sparkles, Trash2 } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
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
  AiProvider,
  AiConversationSummary,
  AiLocalPick,
  AiPreview,
  AiStandaloneSource,
  AiSymptom,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { Choice, StatusText } from '../../settings/components/fields';
import { formatWhen } from '../../test/text';
import { ModelLine } from '../ai/AiPage';
import { useAiSettings } from '../ai/api';
import { originLabel, symptomLabel, tokensLabel } from '../ai/text';
import {
  useChooseStandaloneLog,
  useDeleteStandalone,
  useStandaloneConversations,
  useStandaloneLiveUpdates,
  useStandalonePreview,
} from './api';
import { StandaloneConsent } from './StandaloneConsent';
import '../ai/ai.css';

const SYMPTOMS: readonly AiSymptom[] = ['crashOnStart', 'crashInWorld', 'slow', 'other'];
type Origin = 'link' | 'arquivo';
/** Nomes dos grupos de rádio. */
const SYMPTOM_GROUP = 'ia-sem-pack-sintoma';
const ORIGIN_GROUP = 'ia-sem-pack-origem';

export function StandalonePage() {
  const { t } = useTranslation('iaSemPack');
  const settings = useAiSettings();
  useStandaloneLiveUpdates();

  const head = (
    <PageHead
      title={
        <>
          <Icon icon={Sparkles} size="lg" className="icon--ai" /> {t('pagina.titulo')}
        </>
      }
      sub={t('pagina.sub')}
    />
  );
  let body;
  if (settings.isPending) {
    body = <LoadingState label={t('pagina.carregando')} />;
  } else if (settings.isError) {
    body = <ErrorPanel error={settings.error} />;
  } else if (!settings.data.keyConfigured) {
    body = (
      <>
        {head}
        <MissingKey provider={settings.data.provider} />
      </>
    );
  } else {
    body = (
      <>
        {head}
        <NewAnalysis />
        <ModelLine provider={settings.data.provider} tier={settings.data.tier} />
        <StandaloneList />
      </>
    );
  }
  return (
    <AppPage back={{ to: '/', label: t('pagina.voltar') }} where={t('pagina.titulo')}>
      <div className="stack ai-page">{body}</div>
    </AppPage>
  );
}

function MissingKey({ provider }: { provider: AiProvider }) {
  const { t } = useTranslation('ia');
  return (
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
  );
}

function NewAnalysis() {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const navigate = useNavigate();
  const preview = useStandalonePreview();
  const chooseLog = useChooseStandaloneLog();
  const linkId = useId();
  const noteId = useId();
  const [symptom, setSymptom] = useState<AiSymptom>('crashInWorld');
  const [origin, setOrigin] = useState<Origin>('link');
  const [url, setUrl] = useState('');
  const [pick, setPick] = useState<AiLocalPick | null>(null);
  const [note, setNote] = useState('');
  const [consent, setConsent] = useState<AiPreview | null>(null);
  const opener = useRef<HTMLButtonElement>(null);

  const source: AiStandaloneSource | null =
    origin === 'link'
      ? url.trim() === ''
        ? null
        : { kind: 'link', url: url.trim() }
      : pick
        ? { kind: 'localFile', token: pick.token }
        : null;

  const begin = () => {
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

  const choose = () => {
    chooseLog.mutate(undefined, {
      onSuccess: (picked) => {
        if (picked) setPick(picked);
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  return (
    <section className="panel panel--strong" aria-labelledby="ia-sem-pack-nova">
      <h2 className="panel__title panel__title--sans" id="ia-sem-pack-nova">
        {t('nova.titulo')}
      </h2>
      <fieldset className="ai-new__symptoms">
        <legend className="field__label">{ti('nova.sintoma')}</legend>
        <div className="choice-list choice-list--2">
          {SYMPTOMS.map((value) => (
            <Choice
              key={value}
              name={SYMPTOM_GROUP}
              title={symptomLabel(value)}
              desc={ti(`sintoma.${value}.desc`)}
              checked={symptom === value}
              onSelect={() => {
                setSymptom(value);
              }}
            />
          ))}
        </div>
      </fieldset>
      <fieldset className="ai-new__symptoms">
        <legend className="field__label">{t('nova.origem.rotulo')}</legend>
        <div className="choice-list choice-list--2">
          <Choice
            name={ORIGIN_GROUP}
            title={t('nova.origem.link')}
            desc={t('nova.origem.linkDesc')}
            checked={origin === 'link'}
            onSelect={() => {
              setOrigin('link');
            }}
          />
          <Choice
            name={ORIGIN_GROUP}
            title={t('nova.origem.arquivo')}
            desc={t('nova.origem.arquivoDesc')}
            checked={origin === 'arquivo'}
            onSelect={() => {
              setOrigin('arquivo');
            }}
          />
        </div>
      </fieldset>
      <div className="grid-2 ai-new__fields">
        {origin === 'link' ? (
          <div className="field">
            <label className="field__label" htmlFor={linkId}>
              {t('nova.link.rotulo')}
            </label>
            <input
              id={linkId}
              className="input"
              type="url"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              value={url}
              placeholder={t('nova.link.exemplo')}
              aria-describedby={`${linkId}-dica`}
              onChange={(event) => {
                setUrl(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') begin();
              }}
            />
            <div className="field__hint" id={`${linkId}-dica`}>
              {t('nova.link.dica')}
            </div>
          </div>
        ) : (
          <div className="field">
            <span className="field__label">{t('nova.arquivo.rotulo')}</span>
            <div className="row row--gap-3">
              <Button icon={FileText} loading={chooseLog.isPending} onClick={choose}>
                {pick ? t('nova.arquivo.trocar') : t('nova.arquivo.escolher')}
              </Button>
              {pick ? <span className="t-sm t-strong">{pick.fileName}</span> : null}
            </div>
            <div className="field__hint">
              {pick
                ? t('nova.arquivo.dica', { tamanho: formatBytes(pick.bytes) })
                : t('nova.arquivo.aceitos')}
            </div>
          </div>
        )}
        <div className="field">
          <label className="field__label" htmlFor={noteId}>
            {ti('nova.nota')} <span className="opt">{ti('nova.opcional')}</span>
          </label>
          <textarea
            id={noteId}
            className="textarea"
            rows={2}
            maxLength={2000}
            value={note}
            placeholder={ti('nova.notaExemplo')}
            onChange={(event) => {
              setNote(event.target.value);
            }}
          />
        </div>
      </div>
      {preview.isError ? (
        <Alert kind="danger" compact title={ti('nova.erro')}>
          {appErrorMessage(toAppError(preview.error))}
        </Alert>
      ) : null}
      <div className="row row--gap-3 ai-new__actions">
        <Button
          ref={opener}
          variant="primary"
          icon={Sparkles}
          loading={preview.isPending}
          disabled={source === null}
          onClick={begin}
        >
          {preview.isPending
            ? origin === 'link'
              ? t('nova.baixando')
              : ti('nova.preparando')
            : ti('nova.comecar')}
        </Button>
        <span className="t-sm t-3">{ti('nova.antes')}</span>
      </div>
      {consent ? (
        <StandaloneConsent
          preview={consent}
          returnFocusTo={() => opener.current}
          onClose={() => {
            setConsent(null);
          }}
          onStarted={(id) => {
            setConsent(null);
            void navigate({ to: '/analisar-log/$conversaId', params: { conversaId: id } });
          }}
        />
      ) : null}
    </section>
  );
}

const STATE_KIND: Record<AiConversationState, 'ok' | 'warn' | 'danger' | 'muted'> = {
  running: 'muted',
  answered: 'ok',
  failed: 'danger',
  interrupted: 'warn',
};

function StandaloneList() {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const list = useStandaloneConversations();
  return (
    <section className="stack-2" aria-labelledby="ia-sem-pack-conversas">
      <h2 className="group-title" id="ia-sem-pack-conversas">
        {t('conversas.titulo')}
      </h2>
      {list.isPending ? (
        <LoadingState label={ti('conversas.carregando')} />
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
                <th>{ti('conversas.colunas.conversa')}</th>
                <th>{ti('conversas.colunas.quando')}</th>
                <th>{ti('conversas.colunas.situacao')}</th>
                <th className="shrink">
                  <span className="sr-only">{ti('conversas.colunas.acoes')}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {list.data.map((conversation) => (
                <Row key={conversation.id} conversation={conversation} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="t-xs t-3">{t('conversas.onde')}</p>
    </section>
  );
}

function Row({ conversation }: { conversation: AiConversationSummary }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const remove = useDeleteStandalone();
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
          {ti(`conversas.situacao.${conversation.state}`)}
        </StatusText>
      </td>
      <td className="shrink">
        <div className="row">
          <Button size="sm" asChild>
            <Link
              to="/analisar-log/$conversaId"
              params={{ conversaId: conversation.id }}
              aria-label={ti('conversas.continuarRotulo', { titulo: title })}
            >
              {ti('conversas.continuar')}
            </Link>
          </Button>
          <ConfirmDialog
            trigger={
              <Button size="sm" variant="ghost" iconOnly icon={Trash2}>
                {ti('conversas.apagarRotulo', { titulo: title })}
              </Button>
            }
            title={ti('apagar.titulo')}
            description={t('apagar.texto')}
            confirmLabel={ti('apagar.confirmar')}
            confirmingLabel={ti('apagar.apagando')}
            onConfirm={async () => {
              await remove.mutateAsync(conversation.id);
            }}
          />
        </div>
      </td>
    </tr>
  );
}
