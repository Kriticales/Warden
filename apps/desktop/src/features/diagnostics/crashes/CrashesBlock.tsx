/**
 * Travamentos na seção Problemas (SPEC T14; ROADMAP D-06; esboço aprovado do bug-01,
 * "Separadas"): os testes que travaram agrupados pela causa (assinatura), uma linha por causa
 * com quantas vezes, a última vez e as versões do pack, o selo da situação, "Ver o que causou"
 * (o resultado do teste mais novo do grupo), cada vez que travou, "Encontrar o mod culpado" e
 * a ligação com o Sculk: **Abrir a conversa com o Sculk** quando já há conversa ligada ao
 * travamento, **✦ Conversar com o Sculk** quando não há.
 *
 * A retenção dos travamentos guardados avisa perto do limite e, passando dele, oferece apagar
 * os mais antigos (com confirmação).
 */
import { Link } from '@tanstack/react-router';
import {
  ChevronDown,
  ChevronRight,
  CircleCheck,
  CircleHelp,
  History,
  OctagonX,
} from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { CrashGroupView, CrashRetention, PackId } from '../../../lib/ipc/bindings';
import { CrashBisectAction } from '../../bisect/CrashEntry';
import { formatMemory, formatWhen } from '../../test/text';
import { useAiConversations } from '../ai/api';
import { AskSculkLink, OpenSculkConversation } from '../components/AskSculk';
import type { ProblemsBlockProps } from '../problems-blocks';
import { useCrash, useCrashes, usePruneCrashes } from './api';
import { crashTitle, situationKind, situationText, versionLabel, versionsText } from './text';
import './crashes.css';

const MIB = 1024 * 1024;

export function CrashesBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('travamentos');
  const crashes = useCrashes(packId);
  const conversations = useAiConversations(packId);
  const limite = formatMemory((crashes.data?.retention.budgetBytes ?? 500 * MIB) / MIB);
  const count = crashes.data?.groups.length ?? 0;

  // A conversa mais nova ligada a cada travamento (pela assinatura).
  const linked = new Map<string, string>();
  for (const conversation of [...(conversations.data ?? [])].sort(
    (a, b) => a.updatedAtMs - b.updatedAtMs,
  )) {
    if (conversation.origin.kind === 'session' && conversation.origin.signature) {
      linked.set(conversation.origin.signature, conversation.id);
    }
  }

  return (
    <section className="panel panel--parts" aria-labelledby="diag-crashes-title">
      <div className="panel__bar">
        <h2 className="panel__bar-title" id="diag-crashes-title">
          <Icon icon={OctagonX} />
          {t('titulo')}
        </h2>
        {count > 0 ? <span className="count">{t('quantos', { count })}</span> : null}
      </div>
      {crashes.isPending ? (
        <div className="panel__body">
          <LoadingState label={t('carregando')} inline />
        </div>
      ) : crashes.isError ? (
        <div className="panel__body">
          <ErrorPanel
            title={t('erro')}
            error={crashes.error}
            compact
            onRetry={() => {
              void crashes.refetch();
            }}
          />
        </div>
      ) : (
        <>
          <RetentionNotice packId={packId} retention={crashes.data.retention} limite={limite} />
          {crashes.data.groups.length === 0 ? (
            <div className="panel__body">
              <p className="t-sm t-3">{t('vazio')}</p>
            </div>
          ) : (
            <ul className="rowlist diag-crashes">
              {crashes.data.groups.map((group) => (
                <CrashRow
                  key={group.signature.hash}
                  packId={packId}
                  group={group}
                  conversationId={linked.get(group.signature.hash) ?? null}
                />
              ))}
            </ul>
          )}
          <div className="panel__foot">
            <span className="t-xs t-3">
              {t('subtitulo')}. {t('rodape', { limite })}
            </span>
          </div>
        </>
      )}
    </section>
  );
}

const SITUATION_TAG = { ok: 'tag--ok', danger: 'tag--danger', muted: 'tag--plain' } as const;

function CrashRow({
  packId,
  group,
  conversationId,
}: {
  packId: PackId;
  group: CrashGroupView;
  conversationId: string | null;
}) {
  const { t } = useTranslation('travamentos');
  const [open, setOpen] = useState(false);
  const listId = useId();
  const quando = formatWhen(group.lastAtMs);
  const kind = situationKind(group.situation);
  return (
    <li className={`rowlist__row rowlist__row--${kind === 'muted' ? 'info' : 'danger'}`}>
      <Icon icon={OctagonX} size="md" />
      <h3 className="rowlist__title">{crashTitle(group)}</h3>
      <p className="rowlist__text">
        <span>{t('vezes', { count: group.count })}</span>
        {' · '}
        <span>{t('ultimaVez', { quando })}</span>
        {' · '}
        <span>{versionsText(group.versions)}</span>
      </p>
      <div className="rowlist__actions">
        <span className={`tag ${SITUATION_TAG[kind]} diag-crashes__status`}>
          <Icon icon={kind === 'ok' ? CircleCheck : kind === 'danger' ? CircleHelp : History} />
          {situationText(group.situation)}
        </span>
        <Button size="sm" variant="ghost" asChild>
          <Link
            to="/packs/$packId/teste"
            params={{ packId }}
            search={{ sessao: group.latestSessionId }}
            aria-label={t('verCausaRotulo', { quando })}
          >
            {t('verCausa')}
          </Link>
        </Button>
        {group.count > 1 ? (
          <Button
            size="sm"
            variant="ghost"
            icon={open ? ChevronDown : ChevronRight}
            aria-expanded={open}
            aria-controls={listId}
            onClick={() => {
              setOpen((value) => !value);
            }}
          >
            {open ? t('ocultarVezes') : t('verVezes', { count: group.count })}
          </Button>
        ) : null}
        {/* D-12: "Encontrar o mod culpado" e a busca ligada a este travamento. */}
        <CrashBisectAction packId={packId} group={group} />
        {conversationId ? (
          <OpenSculkConversation
            packId={packId}
            conversationId={conversationId}
            label={t('sculk.abrirRotulo', { quando })}
          >
            {t('sculk.abrir')}
          </OpenSculkConversation>
        ) : (
          <AskSculkLink
            packId={packId}
            subject={{ sessao: group.latestSessionId }}
            label={t('sculk.conversarRotulo', { quando })}
          >
            {t('sculk.conversar')}
          </AskSculkLink>
        )}
      </div>
      {open ? (
        <div className="rowlist__more" id={listId}>
          <Occurrences packId={packId} signature={group.signature.hash} />
        </div>
      ) : null}
    </li>
  );
}

/** Cada vez que o jogo travou com essa causa, com o link para o resultado do teste. */
function Occurrences({ packId, signature }: { packId: PackId; signature: string }) {
  const { t } = useTranslation('travamentos');
  const detail = useCrash(packId, signature, true);
  if (detail.isPending) {
    return <LoadingState label={t('vezesCarregando')} />;
  }
  if (detail.isError) {
    return (
      <ErrorPanel
        title={t('erro')}
        error={detail.error}
        compact
        onRetry={() => {
          void detail.refetch();
        }}
      />
    );
  }
  return (
    <div>
      <p className="t-xs t-3">{t('vezesTitulo')}</p>
      <ul className="diag-crashes__occurrences">
        {(detail.data?.occurrences ?? []).map((occurrence) => {
          const quando = formatWhen(occurrence.startedAtMs);
          return (
            <li key={occurrence.sessionId}>
              <Link
                to="/packs/$packId/teste"
                params={{ packId }}
                search={{ sessao: occurrence.sessionId }}
                aria-label={t('abrirVez', { quando })}
              >
                {t('vez', { quando, versao: versionLabel(occurrence.version) })}
              </Link>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Aviso perto do limite e, passando dele, "Apagar os mais antigos" com confirmação. */
function RetentionNotice({
  packId,
  retention,
  limite,
}: {
  packId: PackId;
  retention: CrashRetention;
  limite: string;
}) {
  const { t } = useTranslation('travamentos');
  const prune = usePruneCrashes(packId);
  const count = retention.prunable.length;
  if (count > 0) {
    return (
      <Alert
        kind="warn"
        className="diag-crashes__notice"
        title={t('retencao.passouTitulo', { limite })}
        actions={
          <ConfirmDialog
            trigger={<Button size="sm">{t('retencao.apagar')}</Button>}
            title={t('retencao.confirmarTitulo', { count })}
            description={t('retencao.confirmarTexto')}
            confirmLabel={t('retencao.confirmar', { count })}
            confirmingLabel={t('retencao.apagando')}
            onConfirm={async () => {
              await prune.mutateAsync(retention.prunable);
            }}
          />
        }
      >
        {t('retencao.passou', { count })}
      </Alert>
    );
  }
  if (retention.overLimit) {
    return (
      <Alert
        kind="warn"
        className="diag-crashes__notice"
        title={t('retencao.passouTitulo', { limite })}
      >
        {t('retencao.passouSemApagar')}
      </Alert>
    );
  }
  if (retention.nearLimit) {
    return (
      <Alert
        kind="info"
        className="diag-crashes__notice"
        title={t('retencao.pertoTitulo', { limite })}
      >
        {t('retencao.perto')}
      </Alert>
    );
  }
  return null;
}
