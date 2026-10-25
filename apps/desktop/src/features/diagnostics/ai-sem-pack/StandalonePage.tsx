/**
 * Analisar um log (ROADMAP D-19; SPEC T14 item 8): a conversa com o Sculk **sem pack**, aberta
 * pelo botão de Meus packs. Página do nível do app (como Configurações), porque a ESTRUTURA não
 * tem lugar para isso dentro de um pack: não há pack. Usa a mesma linguagem do Sensor Sculk
 * (protótipo `design/bug-01/sensor-sculk.html`): conversas à esquerda, a conversa ou a nova
 * análise no meio e a gaveta Detalhes.
 *
 * - **Nova análise:** o sintoma, o log (**um link colado**, do mclo.gs ou do pastebin, ou **um
 *   arquivo do computador**), "Conte o que aconteceu", Como procurar (Claude e OpenAI) e
 *   **Começar conversa**, que baixa o texto do link (só ele vai à rede), tira os dados pessoais
 *   e abre o consentimento. Um link de outro site mostra a frase de recusa sem rede.
 * - Sem a chave do provedor escolhido, o caminho para Configurações, como na seção do pack.
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { FileText, KeyRound, Sparkles } from 'lucide-react';
import { useId, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  AiLocalPick,
  AiPreview,
  AiProvider,
  AiStandaloneSource,
  AiSymptom,
  SearchMode,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useSecretsStatus } from '../../settings/api';
import { useAiSettings } from '../ai/api';
import { ConversationList } from '../ai/ConversationList';
import { ChipRadios, ModeField, SymptomChips } from '../ai/NewConversation';
import { ChatColumn, ChatHead } from '../ai/SculkPage';
import {
  useChooseStandaloneLog,
  useDeleteStandalone,
  useRetryStandalone,
  useSendStandalone,
  useStandaloneConversation,
  useStandaloneConversations,
  useStandaloneLiveUpdates,
  useStandalonePreview,
} from './api';
import { StandaloneConsent } from './StandaloneConsent';
import '../ai/sculk.css';

type Origin = 'link' | 'arquivo';

export function StandalonePage({ conversationId }: { conversationId: string | null }) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const navigate = useNavigate();
  const settings = useAiSettings();
  const secrets = useSecretsStatus();
  const list = useStandaloneConversations();
  const [drawer, setDrawer] = useState(false);
  useStandaloneLiveUpdates();

  const goNew = () => {
    void navigate({ to: '/analisar-log' });
  };
  const goTo = (id: string) => {
    void navigate({ to: '/analisar-log/$conversaId', params: { conversaId: id } });
  };

  const provider = settings.data?.provider ?? null;
  let main: ReactNode;
  if (settings.isPending) {
    main = <LoadingState label={t('pagina.carregando')} />;
  } else if (settings.isError) {
    main = <ErrorPanel error={settings.error} />;
  } else if (conversationId) {
    main = (
      <StandaloneConversation
        key={conversationId}
        conversationId={conversationId}
        drawer={drawer}
        onDrawer={setDrawer}
        onNew={goNew}
        onDeleted={() => {
          const next = (list.data ?? []).find((item) => item.id !== conversationId);
          if (next) goTo(next.id);
          else goNew();
        }}
      />
    );
  } else {
    main = (
      <section className="sculk__chat" aria-label={t('nova.titulo')}>
        <ChatHead title={t('nova.titulo')} />
        <div className="sculk__scroll">
          {settings.data.keyConfigured ? (
            <NewAnalysis provider={settings.data.provider} onStarted={goTo} />
          ) : (
            <div className="newconv">
              <EmptyState
                glyph="dots"
                title={ti(`chave.titulo.${settings.data.provider}`)}
                text={
                  <>
                    {ti('chave.texto')} {ti('chave.provedores')}
                  </>
                }
                actions={
                  <Link to="/configuracoes" className={buttonVariants({ variant: 'primary' })}>
                    <Icon icon={KeyRound} />
                    <span>{ti('chave.abrir')}</span>
                  </Link>
                }
              />
            </div>
          )}
        </div>
      </section>
    );
  }
  return (
    <AppPage back={{ to: '/', label: t('pagina.voltar') }} where={t('pagina.titulo')}>
      <div className="sculk sculk--app" data-testid="analisar-log">
        {/* O título da página, para o leitor de tela (a barra de cima já mostra onde se está). */}
        <h1 className="sr-only">{t('pagina.titulo')}</h1>
        <ConversationList
          conversations={list}
          selectedId={conversationId}
          onNew={goNew}
          renderLink={(id, props) => (
            <Link to="/analisar-log/$conversaId" params={{ conversaId: id }} {...props} />
          )}
          provider={provider}
          keyStorage={secrets.data?.backend ?? null}
          keyConfigured={settings.data?.keyConfigured ?? false}
          empty={{ title: t('conversas.vazio'), hint: t('conversas.onde') }}
        />
        {main}
      </div>
    </AppPage>
  );
}

function StandaloneConversation({
  conversationId,
  drawer,
  onDrawer,
  onNew,
  onDeleted,
}: {
  conversationId: string;
  drawer: boolean;
  onDrawer: (open: boolean) => void;
  onNew: () => void;
  onDeleted: () => void;
}) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const conversation = useStandaloneConversation(conversationId);
  const send = useSendStandalone(conversationId);
  const retry = useRetryStandalone(conversationId);
  const remove = useDeleteStandalone();
  if (conversation.isPending) {
    return (
      <section className="sculk__chat">
        <LoadingState label={ti('conversa.carregando')} />
      </section>
    );
  }
  if (conversation.isError) {
    return (
      <section className="sculk__chat">
        <ChatHead title={ti('conversa.naoAchou')} />
        <div className="newconv">
          <ErrorPanel error={conversation.error} />
        </div>
      </section>
    );
  }
  return (
    <ChatColumn
      packId={null}
      view={conversation.data}
      drawer={drawer}
      onDrawer={onDrawer}
      onNew={onNew}
      onRetry={retry}
      send={{
        isPending: send.isPending,
        error: send.isError ? send.error : null,
        run: (text, mode) => send.mutateAsync({ text, mode }),
      }}
      onDelete={async () => {
        await remove.mutateAsync(conversationId);
        onDrawer(false);
        showToast({ kind: 'ok', title: ti('apagar.apagada') });
        onDeleted();
      }}
      scope="standalone"
      placeholder={t('perguntar.exemplo')}
    />
  );
}

function NewAnalysis({
  provider,
  onStarted,
}: {
  provider: AiProvider;
  onStarted: (id: string) => void;
}) {
  const { t } = useTranslation('iaSemPack');
  const { t: ti } = useTranslation('ia');
  const preview = useStandalonePreview();
  const chooseLog = useChooseStandaloneLog();
  const ids = useId();
  const [symptom, setSymptom] = useState<AiSymptom | null>(null);
  const [origin, setOrigin] = useState<Origin>('link');
  const [url, setUrl] = useState('');
  const [pick, setPick] = useState<AiLocalPick | null>(null);
  const [note, setNote] = useState('');
  const [mode, setMode] = useState<SearchMode>('deep');
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
  const ready = source !== null && symptom !== null;

  const begin = () => {
    if (!source || !symptom || preview.isPending) return;
    preview.mutate(
      {
        symptom,
        source,
        note: note.trim() === '' ? null : note,
        mode: provider === 'gemini' ? null : mode,
      },
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

  const start = (
    <Button
      ref={opener}
      variant="primary"
      icon={Sparkles}
      loading={preview.isPending}
      aria-disabled={!ready}
      onClick={() => {
        if (ready) begin();
      }}
    >
      {preview.isPending
        ? origin === 'link'
          ? t('nova.baixando')
          : ti('nova.preparando')
        : ti('nova.comecar')}
    </Button>
  );

  return (
    <form
      className="newconv"
      noValidate
      aria-labelledby={`${ids}-titulo`}
      onSubmit={(event) => {
        event.preventDefault();
      }}
    >
      <h3 id={`${ids}-titulo`}>{t('nova.titulo')}</h3>
      <p className="newconv__sub">{t('pagina.sub')}</p>
      <div className="fgroup">
        <span className="flabel" id={`${ids}-sintoma`}>
          {ti('nova.sintoma')}
        </span>
        <SymptomChips
          value={symptom}
          onChange={setSymptom}
          labelledBy={`${ids}-sintoma`}
          describedBy={`${ids}-sintoma-dica`}
        />
        <p className="fhint" id={`${ids}-sintoma-dica`}>
          {symptom ? ti(`sintoma.${symptom}.desc`) : ti('nova.sintomaDica')}
        </p>
      </div>
      <div className="fgroup">
        <span className="flabel" id={`${ids}-origem`}>
          {t('nova.origem.rotulo')}
        </span>
        <ChipRadios
          options={[
            { value: 'link', label: t('nova.origem.link') },
            { value: 'arquivo', label: t('nova.origem.arquivo') },
          ]}
          value={origin}
          onChange={setOrigin}
          labelledBy={`${ids}-origem`}
          describedBy={`${ids}-origem-dica`}
        />
        <p className="fhint" id={`${ids}-origem-dica`}>
          {origin === 'link' ? t('nova.origem.linkDesc') : t('nova.origem.arquivoDesc')}
        </p>
      </div>
      {origin === 'link' ? (
        <div className="fgroup">
          <label className="flabel" htmlFor={`${ids}-link`}>
            {t('nova.link.rotulo')}
          </label>
          <input
            id={`${ids}-link`}
            className="input"
            type="url"
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            value={url}
            placeholder={t('nova.link.exemplo')}
            aria-describedby={`${ids}-link-dica`}
            onChange={(event) => {
              setUrl(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                begin();
              }
            }}
          />
          <p className="fhint" id={`${ids}-link-dica`}>
            {t('nova.link.dica')}
          </p>
        </div>
      ) : (
        <div className="fgroup">
          <span className="flabel">{t('nova.arquivo.rotulo')}</span>
          <div className="row row--gap-3">
            <Button icon={FileText} loading={chooseLog.isPending} onClick={choose}>
              {pick ? t('nova.arquivo.trocar') : t('nova.arquivo.escolher')}
            </Button>
            {pick ? <span className="t-sm t-strong">{pick.fileName}</span> : null}
          </div>
          <p className="fhint">
            {pick
              ? t('nova.arquivo.dica', { tamanho: formatBytes(pick.bytes) })
              : t('nova.arquivo.aceitos')}
          </p>
        </div>
      )}
      <div className="fgroup">
        <label className="flabel" htmlFor={`${ids}-nota`}>
          {ti('nova.nota')} <span className="opt">{ti('nova.opcional')}</span>
        </label>
        <textarea
          id={`${ids}-nota`}
          className="textarea"
          maxLength={2000}
          value={note}
          spellCheck={false}
          placeholder={ti('nova.notaExemplo')}
          onChange={(event) => {
            setNote(event.target.value);
          }}
        />
      </div>
      <ModeField provider={provider} mode={mode} onChange={setMode} />
      {preview.isError ? (
        <Alert kind="danger" compact title={ti('nova.erro')}>
          {appErrorMessage(toAppError(preview.error))}
        </Alert>
      ) : null}
      <div className="newconv__go">
        {ready ? (
          start
        ) : (
          <Tooltip content={symptom ? t('nova.semLog') : ti('nova.comecarDica')}>{start}</Tooltip>
        )}
        <p className="fhint">{ti('nova.antes')}</p>
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
            onStarted(id);
          }}
        />
      ) : null}
    </form>
  );
}
