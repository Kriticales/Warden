/**
 * Configurações → Sensor Sculk (T21 e T14; esboço `design/bug-01/configuracoes.html`;
 * ADR-0070): a ordem de preferência dos provedores (arrastar, setas e Alt+↑/↓), com o que o
 * Sculk faz com cada um na conversa nova e, só para consulta, o modelo de cada modo; o aviso de
 * cada chave recusada; e a prévia do campo de pergunta com o seletor de modo.
 *
 * A conversa nova usa o primeiro da ordem com a chave funcionando (com chave e sem recusa no
 * último teste); os outros com chave são "Reserva". Quem decide é o backend (`ai_settings_get`,
 * `provider`); a tela só mostra. Uma conversa já começada continua no provedor dela (ADR-0060).
 * O modo de cada pergunta é escolhido no Sensor Sculk, não aqui.
 */
import { Link, useNavigate } from '@tanstack/react-router';
import type { TFunction } from 'i18next';
import { ArrowDown, ArrowUp, CircleX, GripVertical, Info, Send, Sparkles } from 'lucide-react';
import { useEffect, useRef, useState, type DragEvent, type KeyboardEvent } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import { cn } from '../../../lib/cn';
import type { AiProvider, AiSettingsView, ModelChoices } from '../../../lib/ipc/bindings';
import { toAppError } from '../../../lib/ipc/errors';
import { useAiSettings } from '../../diagnostics/ai/api';
import { useModelsOf, useSetAiOrder } from '../ai/api';
import { aiErrorMessage, providerName } from '../ai/providers';
import { useKeysOverview } from '../api';
import { isUsable, keyOf, moveItem, providerRole, type ProviderRole } from '../keys/model';
import { useWhen } from '../keys/words';
import { SettingsFrame } from '../layout/SettingsFrame';
import { KEYS_PAGE } from '../pages';

export function SculkPage() {
  const { t } = useTranslation('configuracoes');
  const settings = useAiSettings();

  let body;
  if (settings.isPending) {
    body = <LoadingState label={t('carregando')} />;
  } else if (settings.isError) {
    body = (
      <div className="spage__pad">
        <ErrorPanel
          error={settings.error}
          onRetry={() => {
            void settings.refetch();
          }}
        />
      </div>
    );
  } else {
    body = <SculkContent view={settings.data} />;
  }
  return (
    <SettingsFrame page="sensor-sculk" icon={Sparkles}>
      {body}
    </SettingsFrame>
  );
}

function SculkContent({ view }: { view: AiSettingsView }) {
  const { t } = useTranslation('configuracoes');
  const refused = view.order.filter((provider) => providerRole(view, provider) === 'refused');
  return (
    <div className="spage__pad">
      <div>
        <div className="porder__sub">
          <h2>{t('sculk.preferencia')}</h2>
          <p>{t('sculk.preferenciaTexto')}</p>
        </div>
        <ProviderOrder view={view} />
        <p className="porder__note">
          <Icon icon={Info} />
          <span>{t('sculk.nota')}</span>
        </p>
        {refused.length > 0 ? (
          <div className="porder__warns">
            {refused.map((provider) => (
              <RefusedWarning key={provider} provider={provider} />
            ))}
          </div>
        ) : null}
      </div>
      <div className="sculk-low">
        <QuestionPreview view={view} />
        <Alert kind="neutral" role="none">
          <Trans
            t={t}
            i18nKey="sculk.chavesNota"
            components={{
              keys: (
                <Link
                  to="/configuracoes/$pagina"
                  params={{ pagina: KEYS_PAGE }}
                  className="btn btn--link"
                />
              ),
            }}
          />
        </Alert>
      </div>
    </div>
  );
}

/** A lista ordenável. A tela mostra a ordem pedida enquanto o backend grava. */
function ProviderOrder({ view }: { view: AiSettingsView }) {
  const { t } = useTranslation('configuracoes');
  const setOrder = useSetAiOrder();
  const [pending, setPending] = useState<AiProvider[] | null>(null);
  const [dragged, setDragged] = useState<AiProvider | null>(null);
  const [over, setOver] = useState<AiProvider | null>(null);
  const focusAfter = useRef<AiProvider | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const order = pending ?? view.order;

  useEffect(() => {
    const provider = focusAfter.current;
    if (!provider) return;
    focusAfter.current = null;
    listRef.current
      ?.querySelector<HTMLElement>(`[data-provider="${provider}"] .porder__grip`)
      ?.focus();
  });

  const commit = (next: AiProvider[], focus?: AiProvider) => {
    if (next.join() === order.join()) return;
    setPending(next);
    if (focus) focusAfter.current = focus;
    setOrder.mutate(next, {
      onSuccess: (saved) => {
        const text = saved.order
          .map((provider, index) =>
            t('sculk.ordemItem', { pos: index + 1, provedor: providerName(provider) }),
          )
          .join(', ');
        const first = saved.order.find((provider) => isUsable(saved, provider));
        showToast({
          kind: 'ok',
          title: t('sculk.ordemSalva', {
            ordem: text,
            provedor: first ? providerName(first) : t('sculk.nenhum'),
          }),
        });
      },
      onError: (error) => {
        showToast({
          kind: 'danger',
          title: aiErrorMessage(toAppError(error), order[0] ?? 'gemini'),
        });
      },
      onSettled: () => {
        setPending(null);
      },
    });
  };

  const move = (index: number, delta: number) => {
    const provider = order[index];
    commit(moveItem(order, index, delta), provider);
  };

  const onDrop = (event: DragEvent, target: AiProvider) => {
    event.preventDefault();
    setOver(null);
    if (!dragged || dragged === target) return;
    const from = order.indexOf(dragged);
    const to = order.indexOf(target);
    commit(moveItem(order, from, to - from));
    setDragged(null);
  };

  return (
    <div>
      <div className="porder__head" aria-hidden="true">
        <span />
        <span />
        <span>{t('sculk.colunaProvedor')}</span>
        <span>{t('sculk.colunaConversa')}</span>
        <span>
          <span className="mode">
            <i />
            {t('sculk.rapido')}
          </span>
        </span>
        <span>
          <span className="mode mode--deep">
            <i />
            {t('sculk.fundo')}
          </span>
        </span>
        <span />
      </div>
      <div
        className="porder__list"
        role="list"
        aria-label={t('sculk.listaRotulo')}
        aria-busy={setOrder.isPending}
        ref={listRef}
      >
        {order.map((provider, index) => (
          <ProviderRow
            key={provider}
            provider={provider}
            index={index}
            count={order.length}
            view={view}
            dragging={dragged === provider}
            over={over === provider && dragged !== provider}
            onMove={(delta) => {
              move(index, delta);
            }}
            onDragStart={() => {
              setDragged(provider);
            }}
            onDragEnd={() => {
              setDragged(null);
              setOver(null);
            }}
            onDragOver={(event) => {
              event.preventDefault();
              setOver(provider);
            }}
            onDrop={(event) => {
              onDrop(event, provider);
            }}
          />
        ))}
      </div>
    </div>
  );
}

function ProviderRow({
  provider,
  index,
  count,
  view,
  dragging,
  over,
  onMove,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
}: {
  provider: AiProvider;
  index: number;
  count: number;
  view: AiSettingsView;
  dragging: boolean;
  over: boolean;
  onMove: (delta: number) => void;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDragOver: (event: DragEvent) => void;
  onDrop: (event: DragEvent) => void;
}) {
  const { t } = useTranslation('configuracoes');
  const role = providerRole(view, provider);
  const name = providerName(provider);
  const roleText = roleLabel(t, role);
  const onKeyDown = (event: KeyboardEvent) => {
    if (!event.altKey) return;
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      onMove(-1);
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      onMove(1);
    }
  };
  return (
    // Arrastar é só para o mouse; no teclado, a alça (Alt+↑/↓) e as setas fazem o mesmo.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <div
      className={cn(
        'porder__row',
        role === 'use' && 'is-use',
        (role === 'noKey' || role === 'refused') && 'is-skip',
        dragging && 'is-drag',
        over && 'is-over',
      )}
      role="listitem"
      draggable
      data-provider={provider}
      aria-label={t('sculk.linhaRotulo', { pos: index + 1, provedor: name, estado: roleText })}
      onDragStart={(event) => {
        event.dataTransfer.effectAllowed = 'move';
        event.dataTransfer.setData('text/plain', provider);
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      <Tooltip content={t('sculk.arrastar')}>
        {/* A alça: arrastar com o mouse, Alt+↑/↓ com o teclado. */}
        <button
          type="button"
          className="porder__grip"
          aria-label={t('sculk.mover', { provedor: name })}
          onKeyDown={onKeyDown}
        >
          <Icon icon={GripVertical} />
        </button>
      </Tooltip>
      <span className="porder__rank" aria-hidden="true">
        {index + 1}º
      </span>
      <span className="porder__name">
        <b>{name}</b>
        <small>{t(`chaves.tipos.${provider}`)}</small>
      </span>
      <span className="porder__role">
        <RoleTag role={role} />
      </span>
      <ModelCells provider={provider} role={role} />
      <span className="porder__arrows">
        <Button
          size="sm"
          variant="ghost"
          iconOnly
          icon={ArrowUp}
          aria-label={t('sculk.subir', { provedor: name })}
          disabled={index === 0}
          onClick={() => {
            onMove(-1);
          }}
        />
        <Button
          size="sm"
          variant="ghost"
          iconOnly
          icon={ArrowDown}
          aria-label={t('sculk.descer', { provedor: name })}
          disabled={index === count - 1}
          onClick={() => {
            onMove(1);
          }}
        />
      </span>
    </div>
  );
}

function roleLabel(t: TFunction<'configuracoes'>, role: ProviderRole): string {
  switch (role) {
    case 'use':
      return t('sculk.conversasNovas');
    case 'reserve':
      return t('sculk.reserva');
    case 'noKey':
      return t('sculk.puladoSemChave');
    case 'refused':
      return t('sculk.puladoRecusada');
  }
}

function RoleTag({ role }: { role: ProviderRole }) {
  const { t } = useTranslation('configuracoes');
  const text = roleLabel(t, role);
  if (role === 'use') return <span className="tag tag--use">{text}</span>;
  if (role === 'refused') {
    return (
      <span className="tag tag--danger">
        <Icon icon={CircleX} />
        {text}
      </span>
    );
  }
  return <span className="tag">{text}</span>;
}

/** O modelo de cada modo (Gemini: um só, com a troca no limite do dia). Nomes nunca quebram. */
function ModelCells({ provider, role }: { provider: AiProvider; role: ProviderRole }) {
  const { t } = useTranslation('configuracoes');
  const usable = role === 'use' || role === 'reserve';
  const models = useModelsOf(provider, usable);
  const note = (text: string) => <span className="porder__span model__note">{text}</span>;
  if (!usable) {
    if (provider === 'gemini' && role === 'noKey') return note(t('sculk.geminiSemChave'));
    return note(role === 'refused' ? t('sculk.modelosRecusada') : t('sculk.modelosSemChave'));
  }
  if (!models.data) {
    return note(
      models.isError
        ? aiErrorMessage(toAppError(models.error), provider)
        : t('sculk.modelosCarregando'),
    );
  }
  if (provider === 'gemini') {
    return (
      <span className="porder__span model">
        <b>{models.data.economical ?? ''}</b>
        <span className="model__note">
          {models.data.lighter ? (
            <Trans
              t={t}
              i18nKey="sculk.modeloGemini"
              values={{ modelo: models.data.lighter }}
              components={{ m: <span className="nw t-mono" /> }}
            />
          ) : (
            t('sculk.modeloGeminiSemLeve')
          )}
        </span>
      </span>
    );
  }
  return (
    <>
      <ModelName name={models.data.economical} />
      <ModelName name={models.data.careful} />
    </>
  );
}

function ModelName({ name }: { name: string | null }) {
  return (
    <span className="model">
      <b title={name ?? undefined}>{name ?? '—'}</b>
    </span>
  );
}

function RefusedWarning({ provider }: { provider: AiProvider }) {
  const { t } = useTranslation('configuracoes');
  const navigate = useNavigate();
  const overview = useKeysOverview();
  const when = useWhen()(keyOf(overview.data, provider)?.check?.at) ?? '';
  return (
    <Alert
      kind="warn"
      actions={
        <Button
          size="sm"
          onClick={() => {
            void navigate({
              to: '/configuracoes/$pagina',
              params: { pagina: KEYS_PAGE },
              search: { chave: provider, substituir: true },
            });
          }}
        >
          {t('sculk.substituirChave')}
        </Button>
      }
    >
      <strong>{t('sculk.recusadaTitulo', { de: t(`chaves.de.${provider}`), quando: when })}</strong>{' '}
      {t('sculk.recusadaTexto', { o: t(`sculk.pronomes.${provider}`) })}
    </Alert>
  );
}

/** A prévia do campo de pergunta do Sensor Sculk, com o seletor de modo (só para ver). */
function QuestionPreview({ view }: { view: AiSettingsView }) {
  const { t } = useTranslation('configuracoes');
  const provider = isUsable(view, view.provider) ? view.provider : null;
  const models = useModelsOf(provider ?? 'gemini', provider !== null);
  const [deep, setDeep] = useState(true);
  if (!provider) {
    return (
      <Alert kind="warn" role="none">
        {t('sculk.semProvedor')}
      </Alert>
    );
  }
  const gemini = provider === 'gemini';
  const model = modelOf(models.data, gemini ? 'one' : deep ? 'deep' : 'fast');
  return (
    <div role="group" aria-label={t('sculk.previaRotulo')}>
      <p className="qpreview__head">
        <Trans
          t={t}
          i18nKey="sculk.previaTitulo"
          values={{ provedor: providerName(provider) }}
          components={{ b: <b /> }}
        />
      </p>
      <div className="qpreview">
        <div className="qpreview__text">{t('sculk.previaCampo')}</div>
        <div className="qpreview__bar">
          {gemini ? (
            <span className="qpreview__meta">
              {model ? (
                <Trans
                  t={t}
                  i18nKey="sculk.previaGemini"
                  values={{ modelo: model }}
                  components={{ b: <b /> }}
                />
              ) : (
                t('sculk.previaGeminiSemModelo')
              )}
            </span>
          ) : (
            <>
              <div className="segmented" role="radiogroup" aria-label={t('sculk.previaModo')}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={!deep}
                  onClick={() => {
                    setDeep(false);
                  }}
                >
                  {t('sculk.rapido')}
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={deep}
                  onClick={() => {
                    setDeep(true);
                  }}
                >
                  {t('sculk.fundo')}
                </button>
              </div>
              {model ? (
                <span className="qpreview__meta">
                  <Trans
                    t={t}
                    i18nKey="sculk.previaUsa"
                    values={{ modelo: model }}
                    components={{ b: <b /> }}
                  />
                </span>
              ) : null}
            </>
          )}
          <Button size="sm" variant="primary" icon={Send} disabled tabIndex={-1}>
            {t('sculk.previaEnviar')}
          </Button>
        </div>
      </div>
      <p className="qpreview__caption">
        {gemini ? t('sculk.previaLegendaGemini') : t('sculk.previaLegenda')}
      </p>
    </div>
  );
}

function modelOf(models: ModelChoices | undefined, mode: 'one' | 'fast' | 'deep'): string | null {
  if (!models) return null;
  return mode === 'deep' ? models.careful : models.economical;
}
