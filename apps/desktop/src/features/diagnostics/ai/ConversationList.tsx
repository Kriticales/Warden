/**
 * A coluna de conversas do Sensor Sculk (protótipo `design/bug-01/sensor-sculk.html`): título com
 * **Nova conversa**, busca (título e origem, sem diferenciar acentos), grupos por data (Hoje,
 * Ontem, Esta semana, Antes), cada conversa com o selo (Lendo, Respondida, Esperando você,
 * Aplicada, Parou com erro, Sem resposta) e o rodapé com o provedor das conversas novas. As setas
 * para cima e para baixo andam entre as conversas.
 *
 * A mesma coluna serve à seção do pack e ao "Analisar um log" (sem pack): quem usa dá o link de
 * cada conversa e o de Nova conversa.
 */
import { Link } from '@tanstack/react-router';
import { Check, CircleAlert, CircleCheck, Clock, Plus, Search, X } from 'lucide-react';
import { useId, useState, type KeyboardEvent, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import type { AiConversationSummary, AiProvider } from '../../../lib/ipc/bindings';
import { providerName } from '../../settings/ai/providers';
import { SCULK_PAGE } from '../../settings/pages';
import { conversationTag, fold, GROUPS, groupOf, type ConversationTag } from './status';
import { originLabel, symptomLabel } from './text';

export interface ConversationListProps {
  /** As conversas (a consulta da lista). */
  conversations: {
    data: readonly AiConversationSummary[] | undefined;
    isPending: boolean;
    isError: boolean;
    error: unknown;
    refetch: () => unknown;
  };
  /** A conversa aberta (`null`: Nova conversa). */
  selectedId: string | null;
  /** O link de uma conversa (seção do pack ou "Analisar um log"). */
  renderLink: (id: string, props: ConversationLinkProps) => ReactNode;
  /** O botão Nova conversa. */
  onNew: () => void;
  /** O provedor das conversas novas e se a chave dele está no cofre. */
  provider: AiProvider | null;
  keyConfigured: boolean;
  /** O texto da lista vazia (sem pack, outro texto). */
  empty?: { title: string; hint: string } | undefined;
}

/** O que o link de cada conversa recebe. */
export interface ConversationLinkProps {
  className: string;
  'aria-current': 'page' | undefined;
  'aria-label': string;
  onKeyDown: (event: KeyboardEvent<HTMLAnchorElement>) => void;
  children: ReactNode;
}

export function ConversationList({
  conversations,
  selectedId,
  renderLink,
  onNew,
  provider,
  keyConfigured,
  empty,
}: ConversationListProps) {
  const { t } = useTranslation('ia');
  const titleId = useId();
  const searchId = useId();
  const [query, setQuery] = useState('');
  return (
    <aside className="sculk__convs" aria-labelledby={titleId}>
      <div className="colhead">
        <h2 id={titleId}>{t('lista.titulo')}</h2>
        <Button size="sm" icon={Plus} onClick={onNew}>
          {t('lista.nova')}
        </Button>
      </div>
      <div className="sculk__search">
        <Icon icon={Search} />
        <label className="sr-only" htmlFor={searchId}>
          {t('lista.busca')}
        </label>
        <input
          id={searchId}
          className="input"
          type="search"
          placeholder={t('lista.busca')}
          autoComplete="off"
          spellCheck={false}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
        {query ? (
          <Button
            size="sm"
            variant="ghost"
            iconOnly
            icon={X}
            className="sculk__search-clear"
            onClick={() => {
              setQuery('');
              document.getElementById(searchId)?.focus();
            }}
          >
            {t('lista.limpar')}
          </Button>
        ) : null}
      </div>
      <div className="sculk__list">
        {conversations.isPending ? (
          <LoadingState label={t('lista.carregando')} />
        ) : conversations.isError ? (
          <ErrorPanel
            compact
            error={conversations.error}
            onRetry={() => {
              void conversations.refetch();
            }}
          />
        ) : (
          <Groups
            conversations={conversations.data ?? []}
            query={query}
            selectedId={selectedId}
            renderLink={renderLink}
            empty={empty}
            onClear={() => {
              setQuery('');
              document.getElementById(searchId)?.focus();
            }}
          />
        )}
      </div>
      {provider ? (
        <div className="sculk__foot">
          <div>
            {t('lista.rodape.provedor')} <strong>{providerName(provider)}</strong>,{' '}
            {keyConfigured ? t('lista.rodape.keyring') : t('lista.rodape.semChave')}
          </div>
          <Link to="/configuracoes/$pagina" params={{ pagina: SCULK_PAGE }} className="link">
            {t('lista.rodape.mudar')}
          </Link>
        </div>
      ) : null}
    </aside>
  );
}

/** A classe de cada conversa na lista. */
const CONV_CLASS = 'conv';

const TAG_CLASS: Record<ConversationTag, string> = {
  lendo: 'tag tag--ai',
  respondida: 'tag',
  esperando: 'tag tag--warn',
  aplicada: 'tag tag--ok',
  erro: 'tag tag--danger',
  interrompida: 'tag tag--warn',
};

/** O selo da conversa, com ícone e palavra (nunca só cor). */
export function TagBadge({ tag }: { tag: ConversationTag }) {
  const { t } = useTranslation('ia');
  const icon =
    tag === 'lendo' ? (
      <Loader />
    ) : (
      <Icon
        icon={
          tag === 'aplicada'
            ? CircleCheck
            : tag === 'esperando' || tag === 'interrompida'
              ? Clock
              : tag === 'erro'
                ? CircleAlert
                : Check
        }
        size="sm"
      />
    );
  return (
    <span className={TAG_CLASS[tag]}>
      {icon}
      {t(`estado.${tag}`)}
    </span>
  );
}

/** O título da conversa na lista e no topo: a causa provável ou o sintoma. */
export function conversationTitle(summary: AiConversationSummary): string {
  return summary.title ?? symptomLabel(summary.symptom);
}

function Groups({
  conversations,
  query,
  selectedId,
  renderLink,
  empty,
  onClear,
}: {
  conversations: readonly AiConversationSummary[];
  query: string;
  selectedId: string | null;
  renderLink: ConversationListProps['renderLink'];
  empty: ConversationListProps['empty'];
  onClear: () => void;
}) {
  const { t } = useTranslation('ia');
  // O agrupamento por data fica fixo enquanto a lista está aberta.
  const [now] = useState(() => Date.now());
  if (conversations.length === 0) {
    return (
      <div className="sculk__empty">
        <p>{empty?.title ?? t('lista.vazio')}</p>
        <p>{empty?.hint ?? t('lista.vazioDica')}</p>
      </div>
    );
  }
  const wanted = fold(query.trim());
  const hits = conversations.filter(
    (summary) =>
      !wanted ||
      fold(`${conversationTitle(summary)} ${originLabel(summary.origin)}`).includes(wanted),
  );
  if (hits.length === 0) {
    return (
      <div className="sculk__empty" role="status">
        <p>{t('lista.semResultado', { busca: query.trim() })}</p>
        <p>
          <Button variant="link" onClick={onClear}>
            {t('lista.limpar')}
          </Button>
        </p>
      </div>
    );
  }
  // As setas andam entre as conversas, como numa lista.
  const onKeyDown = (event: KeyboardEvent<HTMLAnchorElement>) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    const all = Array.from(
      event.currentTarget.closest('.sculk__list')?.querySelectorAll<HTMLAnchorElement>('.conv') ??
        [],
    );
    const next = all[all.indexOf(event.currentTarget) + (event.key === 'ArrowDown' ? 1 : -1)];
    if (next) {
      event.preventDefault();
      next.focus();
    }
  };
  return (
    <>
      {GROUPS.map((group) => {
        const items = hits.filter((summary) => groupOf(summary.updatedAtMs, now) === group);
        if (items.length === 0) return null;
        return (
          <div key={group} role="group" aria-label={t(`lista.grupos.${group}`)}>
            <div className="sculk__group" aria-hidden="true">
              {t(`lista.grupos.${group}`)}
            </div>
            {items.map((summary) => {
              const title = conversationTitle(summary);
              const origin = originLabel(summary.origin);
              const tag = conversationTag(summary);
              return (
                <div key={summary.id}>
                  {renderLink(summary.id, {
                    className: CONV_CLASS,
                    'aria-current': summary.id === selectedId ? 'page' : undefined,
                    'aria-label': t('lista.rotulo', {
                      titulo: title,
                      origem: origin,
                      estado: t(`estado.${tag}`),
                    }),
                    onKeyDown,
                    children: (
                      <>
                        <span className="conv__title">{title}</span>
                        <span className="conv__meta">{origin}</span>
                        <TagBadge tag={tag} />
                      </>
                    ),
                  })}
                </div>
              );
            })}
          </div>
        );
      })}
    </>
  );
}
