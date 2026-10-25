/**
 * Sensor Sculk, a seção de IA do pack (SPEC T14; ESTRUTURA §13; protótipo aprovado
 * `design/bug-01/sensor-sculk.html`, versão 2 "Clássico de desktop"; ADR-0064): a coluna de
 * conversas à esquerda, a conversa (ou Nova conversa) no meio e a gaveta **Detalhes**
 * recolhível à direita, usando a área inteira do pack (o menu fica só com os ícones).
 *
 * Sem a chave do provedor ativo (D-18: Gemini, OpenAI ou Claude), o meio mostra o caminho para
 * Configurações, citando os três provedores, com o Gemini sugerido; as conversas continuam na
 * lista, e cada uma continua no provedor em que começou.
 */
import { Link, useNavigate } from '@tanstack/react-router';
import { KeyRound, PanelRight } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PAGE_TITLE_ATTRIBUTE } from '../../../app/layout/focus';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type {
  AiConversationView,
  AiProvider,
  AiSymptom,
  PackId,
  SearchMode,
} from '../../../lib/ipc/bindings';
import { providerName } from '../../settings/ai/providers';
import { KEYS_PAGE } from '../../settings/pages';
import { useSecretsStatus } from '../../settings/api';
import { formatWhen } from '../../test/text';
import {
  useAiConversation,
  useAiConversations,
  useAiLiveUpdates,
  useAiSettings,
  useDeleteConversation,
  useRetryConversation,
  useSendQuestion,
} from './api';
import { Composer } from './Composer';
import { ConversationList, conversationTitle } from './ConversationList';
import { DetailsDrawer, DRAWER_ID } from './DetailsDrawer';
import { NewConversation } from './NewConversation';
import { lastMode, noticesOf } from './status';
import { Thread } from './Thread';
import './sculk.css';

export function SculkPage({
  packId,
  conversationId,
  sessionId,
  subject = null,
  symptom = null,
}: {
  packId: PackId;
  conversationId: string | null;
  sessionId: string | null;
  /** O assunto vindo de "✦ Perguntar ao Sculk" em Problemas (só na conversa nova). */
  subject?: string | null;
  symptom?: AiSymptom | null;
}) {
  const { t } = useTranslation('ia');
  const navigate = useNavigate();
  const secrets = useSecretsStatus();
  const settings = useAiSettings();
  const list = useAiConversations(packId);
  const [drawer, setDrawer] = useState(false);
  useAiLiveUpdates(packId);

  const goNew = () => {
    void navigate({ to: '/packs/$packId/ia', params: { packId } });
  };
  const goTo = (id: string) => {
    void navigate({ to: '/packs/$packId/ia/$conversaId', params: { packId, conversaId: id } });
  };

  const provider = settings.data?.provider ?? null;
  const keyConfigured = provider !== null && (secrets.data?.[provider] ?? false);

  let main: ReactNode;
  if (secrets.isPending || settings.isPending) {
    main = <LoadingState label={t('secao.carregando')} />;
  } else if (secrets.isError || settings.isError) {
    main = (
      <ErrorPanel
        error={secrets.error ?? settings.error}
        onRetry={() => {
          void secrets.refetch();
          void settings.refetch();
        }}
      />
    );
  } else if (conversationId) {
    main = (
      <ConversationColumn
        key={conversationId}
        packId={packId}
        conversationId={conversationId}
        drawer={drawer}
        onDrawer={setDrawer}
        onNew={goNew}
        onDeleted={() => {
          const rest = (list.data ?? []).filter((item) => item.id !== conversationId);
          const next = rest[0];
          if (next) goTo(next.id);
          else goNew();
        }}
      />
    );
  } else {
    main = (
      <NewColumn
        key={`${sessionId ?? ''}|${subject ?? ''}|${symptom ?? ''}`}
        packId={packId}
        sessionId={sessionId}
        subject={subject}
        symptom={symptom}
        provider={settings.data.provider}
        keyConfigured={keyConfigured}
        onStarted={goTo}
      />
    );
  }

  return (
    <div className="sculk" data-testid="sensor-sculk">
      <ConversationList
        conversations={list}
        selectedId={conversationId}
        onNew={goNew}
        renderLink={(id, props) => (
          <Link to="/packs/$packId/ia/$conversaId" params={{ packId, conversaId: id }} {...props} />
        )}
        provider={provider}
        keyConfigured={keyConfigured}
      />
      {main}
    </div>
  );
}

/** O topo da coluna do meio: o título (recebe o foco ao trocar de tela) e o que vier ao lado. */
export function ChatHead({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="colhead">
      <h2 className="sculk__title" title={title} tabIndex={-1} {...{ [PAGE_TITLE_ATTRIBUTE]: '' }}>
        {title}
      </h2>
      {children}
    </div>
  );
}

function NewColumn({
  packId,
  sessionId,
  subject,
  symptom,
  provider,
  keyConfigured,
  onStarted,
}: {
  packId: PackId;
  sessionId: string | null;
  subject: string | null;
  symptom: AiSymptom | null;
  provider: AiProvider;
  keyConfigured: boolean;
  onStarted: (id: string) => void;
}) {
  const { t } = useTranslation('ia');
  return (
    <section className="sculk__chat" aria-label={t('nova.titulo')}>
      <ChatHead title={t('nova.titulo')} />
      <div className="sculk__scroll">
        {keyConfigured ? (
          <NewConversation
            packId={packId}
            sessionId={sessionId}
            subject={subject}
            initialSymptom={symptom}
            provider={provider}
            onStarted={onStarted}
          />
        ) : (
          <div className="newconv">
            <EmptyState
              glyph="dots"
              title={t(`chave.titulo.${provider}`)}
              text={
                <>
                  {t('chave.texto')} {t('chave.provedores')}
                </>
              }
              actions={
                <Link
                  to="/configuracoes/$pagina"
                  params={{ pagina: KEYS_PAGE }}
                  search={{ chave: provider }}
                  className={buttonVariants({ variant: 'primary' })}
                >
                  <Icon icon={KeyRound} />
                  <span>{t('chave.abrir')}</span>
                </Link>
              }
            />
          </div>
        )}
      </div>
    </section>
  );
}

/** O botão Detalhes, com o número de avisos da conversa. */
export function DetailsButton({
  open,
  count,
  onToggle,
}: {
  open: boolean;
  count: number;
  onToggle: () => void;
}) {
  const { t } = useTranslation('ia');
  return (
    <Button
      size="sm"
      icon={PanelRight}
      aria-expanded={open}
      aria-controls={DRAWER_ID}
      aria-label={count > 0 ? t('detalhes.botaoAvisos', { count }) : t('detalhes.botao')}
      onClick={onToggle}
    >
      {t('detalhes.botao')}
      {count > 0 ? (
        <span className="count count--warn" aria-hidden="true">
          {count}
        </span>
      ) : null}
    </Button>
  );
}

/**
 * Sem a chave do provedor da conversa: o pedido dessa chave, com o caminho para Configurações
 * (CA-T14-21; o estado das chaves é o de Configurações, sempre atual).
 */
export function KeyMissing({ provider }: { provider: AiProvider }) {
  const { t } = useTranslation('ia');
  const secrets = useSecretsStatus();
  if (!secrets.data || secrets.data[provider]) {
    return null;
  }
  return (
    <Alert
      kind="warn"
      title={t(`chave.titulo.${provider}`)}
      actions={
        <Button size="sm" icon={KeyRound} asChild>
          <Link
            to="/configuracoes/$pagina"
            params={{ pagina: KEYS_PAGE }}
            search={{ chave: provider }}
          >
            {t('chave.abrir')}
          </Link>
        </Button>
      }
    >
      {t('faltaChave.texto', { provedor: providerName(provider) })}
    </Alert>
  );
}

/** A conversa aberta: topo, mensagens, campo de pergunta e a gaveta Detalhes. */
export function ChatColumn({
  packId,
  view,
  drawer,
  onDrawer,
  onNew,
  onRetry,
  send,
  onDelete,
  scope,
  placeholder,
}: {
  packId: PackId | null;
  view: AiConversationView;
  drawer: boolean;
  onDrawer: (open: boolean) => void;
  onNew: () => void;
  onRetry: () => void;
  send: {
    isPending: boolean;
    error: unknown;
    run: (text: string, mode: SearchMode | null) => Promise<unknown>;
  };
  onDelete: () => Promise<void>;
  scope: 'pack' | 'standalone';
  placeholder?: string | undefined;
}) {
  const { t } = useTranslation('ia');
  const notices = noticesOf(view, view.provider);
  const last = view.turns.at(-1);
  return (
    <>
      <section className="sculk__chat" aria-label={conversationTitle(view.summary)}>
        <ChatHead title={conversationTitle(view.summary)}>
          <span className="sculk__meta">
            {t('conversa.meta', {
              provedor: providerName(view.provider),
              quando: formatWhen(view.summary.createdAtMs),
            })}
          </span>
          <DetailsButton
            open={drawer}
            count={notices.length}
            onToggle={() => {
              onDrawer(!drawer);
            }}
          />
        </ChatHead>
        <div className="sculk__scroll">
          <Thread packId={packId} view={view} onRetry={onRetry} />
        </div>
        <div className="composer__key">
          <KeyMissing provider={view.provider} />
        </div>
        <Composer
          key={view.summary.id}
          provider={view.provider}
          reading={view.live !== null}
          waitingAnswer={!last?.answer && view.live === null}
          initialMode={lastMode(view.turns)}
          sending={send.isPending}
          error={send.error}
          onSend={send.run}
          placeholder={placeholder}
        />
      </section>
      {drawer ? (
        <DetailsDrawer
          view={view}
          notices={notices}
          scope={scope}
          onClose={() => {
            onDrawer(false);
            requestAnimationFrame(() =>
              document.querySelector<HTMLElement>(`[aria-controls="${DRAWER_ID}"]`)?.focus(),
            );
          }}
          onDelete={onDelete}
          onNew={onNew}
        />
      ) : null}
    </>
  );
}

function ConversationColumn({
  packId,
  conversationId,
  drawer,
  onDrawer,
  onNew,
  onDeleted,
}: {
  packId: PackId;
  conversationId: string;
  drawer: boolean;
  onDrawer: (open: boolean) => void;
  onNew: () => void;
  onDeleted: () => void;
}) {
  const { t } = useTranslation('ia');
  const conversation = useAiConversation(packId, conversationId);
  const send = useSendQuestion(packId, conversationId);
  const { retry } = useRetryConversation(packId, conversationId);
  const remove = useDeleteConversation(packId);
  if (conversation.isPending) {
    return (
      <section className="sculk__chat">
        <LoadingState label={t('conversa.carregando')} />
      </section>
    );
  }
  if (conversation.isError) {
    return (
      <section className="sculk__chat">
        <ChatHead title={t('conversa.naoAchou')} />
        <div className="newconv">
          <ErrorPanel error={conversation.error} />
        </div>
      </section>
    );
  }
  return (
    <ChatColumn
      packId={packId}
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
        showToast({ kind: 'ok', title: t('apagar.apagada') });
        onDeleted();
      }}
      scope="pack"
    />
  );
}
