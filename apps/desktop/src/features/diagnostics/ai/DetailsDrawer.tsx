/**
 * A gaveta **Detalhes** da conversa (protótipo `design/bug-01/sensor-sculk.html`): empurra a
 * conversa para o lado e fecha com Esc ou pelo X.
 *
 * - **Serviço e uso:** provedor, modelo (e quantas respostas usaram o mais leve), o modo de cada
 *   pergunta (ADR-0064; "Não se aplica ao Gemini"), tokens contados pelo provedor, pedidos e
 *   quando começou;
 * - **O Sculk pode ler:** o que o consentimento permitiu nesta conversa;
 * - **Tudo o que foi enviado:** o texto inicial, cada pergunta e cada consulta com o tamanho; um
 *   clique leva ao bloco na conversa;
 * - **Avisos desta conversa:** o pack mudou (com Começar nova conversa), a troca para o modelo
 *   mais leve e o aviso do plano gratuito do Gemini;
 * - **Apagar esta conversa**, com a confirmação no próprio rodapé.
 */
import { Check, Trash2, X } from 'lucide-react';
import { Fragment, useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { AiConversationView } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { formatWhen } from '../../test/text';
import { providerName } from '../../settings/ai/providers';
import { modesUsed, turnModel, type ConversationNotice } from './status';
import { byteLength, stepLabel, tokensLabel } from './text';
import { consultDomId, initialDomId, revealConsult, revealInitial } from './Thread';

/** Id da gaveta (o botão Detalhes a controla). */
export const DRAWER_ID = 'sculk-detalhes';

export interface DetailsDrawerProps {
  view: AiConversationView;
  notices: readonly ConversationNotice[];
  /** O que o consentimento lista além do log (sem pack: só o log e o GitHub). */
  scope: 'pack' | 'standalone';
  onClose: () => void;
  /** Apaga a conversa (depois da confirmação). */
  onDelete: () => Promise<void>;
  /** "Começar nova conversa" do aviso de pack mudado. */
  onNew: () => void;
}

export function DetailsDrawer({
  view,
  notices,
  scope,
  onClose,
  onDelete,
  onNew,
}: DetailsDrawerProps) {
  const { t } = useTranslation('ia');
  const titleId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const askRef = useRef<HTMLButtonElement>(null);
  const yesRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    heading.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      // Um diálogo aberto por cima cuida do Esc dele.
      if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      if (confirming) {
        setConfirming(false);
        requestAnimationFrame(() => askRef.current?.focus());
      } else {
        onClose();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
    };
  }, [confirming, onClose]);

  const used = modesUsed(view.turns);
  const lighter = view.turns.filter((turn) => turn.fallbackModel);
  const mode =
    view.provider === 'gemini'
      ? t('detalhes.modoGemini')
      : used.quick > 0 && used.deep > 0
        ? t('detalhes.modoMisto', { rapido: used.quick, fundo: used.deep })
        : used.quick > 0
          ? t('modo.quick')
          : used.deep > 0
            ? t('modo.deep')
            : t('detalhes.modoAntigo');

  const initial = view.turns[0];
  const sent: { key: string; name: string; bytes: number; go: () => void }[] = [];
  view.turns.forEach((turn, turnIndex) => {
    sent.push({
      key: `q-${String(turnIndex)}`,
      name:
        turnIndex === 0
          ? t('detalhes.textoInicial')
          : `${t('conversa.voce')}: ${turn.question.slice(0, 80)}`,
      bytes: byteLength(turn.question),
      go: () => {
        if (turnIndex === 0) revealInitial(initialDomId(view.summary.id));
      },
    });
    turn.steps.forEach((step, stepIndex) => {
      sent.push({
        key: `s-${String(turnIndex)}-${String(stepIndex)}`,
        name: stepLabel(step),
        bytes: byteLength(step.sent),
        go: () => {
          revealConsult(consultDomId(view.summary.id, turnIndex, stepIndex));
        },
      });
    });
  });
  const total = sent.reduce((sum, item) => sum + item.bytes, 0);
  const perms = view.permissions;

  return (
    <aside className="sculk__drawer" id={DRAWER_ID} aria-labelledby={titleId}>
      <div className="colhead">
        <h2 id={titleId} ref={heading} tabIndex={-1} className="sculk__title-plain">
          {t('detalhes.titulo')}
        </h2>
        <Button size="sm" variant="ghost" iconOnly icon={X} onClick={onClose}>
          {t('detalhes.fechar')}
        </Button>
      </div>
      <div className="sculk__drawer-body">
        <section className="dsec" aria-labelledby={`${titleId}-uso`}>
          <h3 id={`${titleId}-uso`}>{t('detalhes.servico')}</h3>
          <dl className="kv">
            <dt>{t('detalhes.provedor')}</dt>
            <dd>{providerName(view.provider)}</dd>
            <dt>{t('detalhes.modelo')}</dt>
            <dd>
              <span className="mono">
                {initial ? (initial.model ?? view.model) : view.model}
              </span>
              {lighter.length > 0 ? (
                <span className="sub">
                  {t('detalhes.modeloLeve', {
                    count: lighter.length,
                    modelo: lighter[0] ? turnModel(lighter[0], view.model) : '',
                  })}
                </span>
              ) : null}
            </dd>
            <dt>{t('detalhes.modo')}</dt>
            <dd>{mode}</dd>
            <dt>{t('detalhes.tokens')}</dt>
            <dd>
              <span className="mono">{tokensLabel(view.summary.tokens)}</span>
              <span className="sub">{t('detalhes.tokensSub')}</span>
            </dd>
            <dt>{t('detalhes.pedidos')}</dt>
            <dd>
              <span className="mono">{view.requests}</span>
            </dd>
            <dt>{t('detalhes.comecou')}</dt>
            <dd>{formatWhen(view.summary.createdAtMs)}</dd>
          </dl>
        </section>
        <section className="dsec" aria-labelledby={`${titleId}-ler`}>
          <h3 id={`${titleId}-ler`}>{t('detalhes.podeLer')}</h3>
          <ul className="perms">
            {scope === 'pack' ? (
              <Perm allowed label={t('detalhes.leituras.pack')} />
            ) : null}
            <Perm allowed label={t('detalhes.leituras.logs')} sub={t('detalhes.leituras.logsSub')} />
            {scope === 'pack' ? (
              <>
                <Perm allowed label={t('detalhes.leituras.desempenho')} />
                <Perm allowed={perms.configs} label={t('detalhes.leituras.configs')} />
              </>
            ) : null}
            <Perm
              allowed={perms.github}
              label={t('detalhes.leituras.github')}
              sub={perms.github ? t('detalhes.leituras.githubSub') : undefined}
            />
          </ul>
        </section>
        <section className="dsec" aria-labelledby={`${titleId}-enviado`}>
          <h3 id={`${titleId}-enviado`}>{t('detalhes.enviados')}</h3>
          <ul className="sent">
            {sent.map((item) => (
              <li key={item.key}>
                <button type="button" title={item.name} onClick={item.go}>
                  <span className="sent__name">{item.name}</span>
                  <span className="sent__size">{formatBytes(item.bytes)}</span>
                </button>
              </li>
            ))}
          </ul>
          <div className="sent__total">
            <span>{t('detalhes.total')}</span>
            <span className="sent__size">{formatBytes(total)}</span>
          </div>
        </section>
        {notices.length > 0 ? (
          <section className="dsec" aria-labelledby={`${titleId}-avisos`}>
            <h3 id={`${titleId}-avisos`}>{t('detalhes.avisos')}</h3>
            <div className="notices">
              {notices.map((notice, index) => (
                <Fragment key={`${notice.kind}-${String(index)}`}>
                  <Notice view={view} notice={notice} onNew={onNew} />
                </Fragment>
              ))}
            </div>
          </section>
        ) : null}
      </div>
      <div className="sculk__drawer-foot">
        {confirming ? (
          <div className="confirm" role="group" aria-labelledby={`${titleId}-apagar`}>
            <strong id={`${titleId}-apagar`}>{t('apagar.titulo')}</strong>
            <p>{t('apagar.texto')}</p>
            <div className="confirm__actions">
              <Button
                ref={yesRef}
                size="sm"
                variant="danger"
                icon={Trash2}
                loading={deleting}
                onClick={() => {
                  setDeleting(true);
                  onDelete().catch(() => {
                    setDeleting(false);
                  });
                }}
              >
                {deleting ? t('apagar.apagando') : t('apagar.confirmar')}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setConfirming(false);
                  requestAnimationFrame(() => askRef.current?.focus());
                }}
              >
                {t('apagar.cancelar')}
              </Button>
            </div>
          </div>
        ) : (
          <Button
            ref={askRef}
            size="sm"
            variant="danger-ghost"
            icon={Trash2}
            onClick={() => {
              setConfirming(true);
              requestAnimationFrame(() => yesRef.current?.focus());
            }}
          >
            {t('detalhes.apagar')}
          </Button>
        )}
      </div>
    </aside>
  );
}

function Perm({
  allowed,
  label,
  sub,
}: {
  allowed: boolean;
  label: string;
  sub?: string | undefined;
}) {
  const { t } = useTranslation('ia');
  return (
    <li className={allowed ? undefined : 'off'}>
      <Icon icon={allowed ? Check : X} className={allowed ? 'yes' : 'no'} />
      <span>{label}</span>
      {allowed ? (sub ? <small>{sub}</small> : null) : <small>{t('detalhes.leituras.negado')}</small>}
    </li>
  );
}

function Notice({
  view,
  notice,
  onNew,
}: {
  view: AiConversationView;
  notice: ConversationNotice;
  onNew: () => void;
}) {
  const { t } = useTranslation('ia');
  if (notice.kind === 'mudou') {
    const changes = view.packChanges;
    const parts: string[] = [];
    if (changes.updated > 0) parts.push(t('mudou.atualizados', { count: changes.updated }));
    if (changes.added > 0) parts.push(t('mudou.adicionados', { count: changes.added }));
    if (changes.removed > 0) parts.push(t('mudou.removidos', { count: changes.removed }));
    const quando = formatWhen(view.summary.createdAtMs);
    return (
      <Alert
        kind="warn"
       
        title={t('mudou.titulo')}
        actions={
          <Button size="sm" onClick={onNew}>
            {t('mudou.nova')}
          </Button>
        }
        actionsBelow
      >
        {parts.length > 0
          ? t('mudou.itens', { mudancas: parts.join(', '), quando })
          : t('mudou.outros', { quando })}
      </Alert>
    );
  }
  if (notice.kind === 'troca') {
    return (
      <Alert kind="neutral" compact>
        {t(`detalhes.${notice.reason}`, { count: notice.count, modelo: notice.model })}
      </Alert>
    );
  }
  return (
    <Alert kind="neutral" compact>
      {t('detalhes.planoGratuito')}
    </Alert>
  );
}
