/**
 * Etapa 4 do Criar pack, "Mods iniciais" (SPEC T03; protótipo `criar-4`; ADR-0033; D16):
 * spark e Crash Assistant já marcados, cada um com para que serve e a versão que será usada, e
 * o kit de desempenho opcional, desmarcado. Nada é escrito aqui: a escolha vai no rascunho e
 * entra pelo mesmo caminho de "Adicionar" depois do "Pack criado".
 *
 * Sem internet, a etapa avisa e deixa seguir sem os mods. Sem chave da CurseForge, o item que só
 * existe lá (spark antigo) aparece desabilitado com o motivo. Pack vanilla não tem mods.
 */
import { ChevronDown, ChevronRight, RefreshCw, TriangleAlert } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { LoadingState } from '../../../../components/common/LoadingState';
import { Alert } from '../../../../components/ui/alert';
import { Button } from '../../../../components/ui/button';
import { Icon } from '../../../../components/ui/icon';
import type { Kit, Loader, OfferItem } from '../../../../lib/ipc/bindings';
import { useKits } from '../../../add/kits/api';
import { KitItems } from '../../../add/kits/KitItems';
import { SourceMark } from '../../../add/common/text';
import { loaderName } from '../../lib/pack-list';
import { useInitialOffer } from './api';
import {
  defaultChoice,
  type InitialChoice,
  NO_INITIAL,
  selectable,
  withKit,
  withKitProject,
  withTool,
} from './model';

export interface InitialModsStepProps {
  minecraft: string;
  loader: Loader | null;
  /** A escolha atual; `null` até a oferta chegar (a etapa preenche com o padrão). */
  choice: InitialChoice | null;
  onChange: (choice: InitialChoice) => void;
}

/** Descrição de uma ferramenta conhecida; ferramenta nova ainda sem texto não mostra frase. */
function toolKey(id: string): 'spark' | 'crash-assistant' | null {
  return id === 'spark' || id === 'crash-assistant' ? id : null;
}

export function InitialModsStep({ minecraft, loader, choice, onChange }: InitialModsStepProps) {
  const { t } = useTranslation('modsIniciais');
  const offer = useInitialOffer(minecraft, loader);
  const kits = useKits(minecraft, loader);

  // A escolha nasce com o padrão da oferta; sem oferta (vanilla, sem internet), sem nada.
  useEffect(() => {
    if (choice !== null) return;
    if (loader === null || offer.isError) onChange(NO_INITIAL);
    else if (offer.data) onChange(defaultChoice(offer.data));
  }, [choice, loader, offer.isError, offer.data, onChange]);

  if (loader === null) {
    return (
      <div className="wizard__body--wide stack">
        <Alert kind="info" title={t('semLoader.titulo')}>
          <p>{t('semLoader.texto')}</p>
        </Alert>
      </div>
    );
  }

  const current = choice ?? NO_INITIAL;
  return (
    <div className="wizard__body--wide stack">
      <p className="t-sm t-2">{t('intro')}</p>
      {offer.isPending ? (
        <LoadingState label={t('carregando')} />
      ) : offer.isError ? (
        <Alert
          kind="warn"
          title={t('erro.titulo')}
          actions={
            <Button
              size="sm"
              icon={RefreshCw}
              loading={offer.isFetching}
              onClick={() => {
                void offer.refetch();
              }}
            >
              {t('erro.tentar')}
            </Button>
          }
        >
          <p>{t('erro.texto')}</p>
        </Alert>
      ) : offer.data.items.length === 0 ? (
        <Alert kind="info" title={t('nenhum.titulo')}>
          <p>{t('nenhum.texto')}</p>
        </Alert>
      ) : (
        <fieldset className="fieldset-bare">
          <legend className="field__label">
            {t('recomendados', { minecraft, loader: loaderName(loader) })}
          </legend>
          <div className="dlist" role="list">
            {offer.data.items.map((item) => (
              <ToolRow
                key={item.id}
                item={item}
                checked={current.tools.includes(item.id)}
                onChange={(on) => {
                  onChange(withTool(current, item.id, on));
                }}
              />
            ))}
          </div>
        </fieldset>
      )}
      <fieldset className="fieldset-bare">
        <legend className="field__label">
          {t('kit.legenda')} <span className="opt">{t('kit.opcional')}</span>
        </legend>
        {kits.data && kits.data.length > 0 ? (
          <div className="dlist" role="list">
            {kits.data.map((kit) => (
              <KitChoiceRow key={kit.id} kit={kit} choice={current} onChange={onChange} />
            ))}
          </div>
        ) : kits.isPending ? null : (
          <p className="t-sm t-3">{t('kit.nenhum')}</p>
        )}
      </fieldset>
      <p className="field__hint">{t('kit.confirma')}</p>
    </div>
  );
}

function ToolRow({
  item,
  checked,
  onChange,
}: {
  item: OfferItem;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  const { t } = useTranslation('modsIniciais');
  const key = toolKey(item.id);
  const disabled = !selectable(item);
  const reason =
    item.unavailable === 'curseforgeOff'
      ? { label: t('semChave'), hint: t('semChaveDica') }
      : item.unavailable === 'noVersion'
        ? { label: t('semVersao'), hint: null }
        : null;
  return (
    <div
      className={disabled ? 'drow drow--compact drow--noversion' : 'drow drow--compact'}
      role="listitem"
    >
      <span className="drow__box">
        <label className="check">
          <input
            type="checkbox"
            checked={checked && !disabled}
            disabled={disabled}
            aria-label={t('selecionar', { name: key ? t(`ferramenta.${key}.nome`) : item.title })}
            onChange={(event) => {
              onChange(event.target.checked);
            }}
          />
        </label>
      </span>
      <span className="drow__main">
        <span className="drow__line">
          <b>{key ? t(`ferramenta.${key}.nome`) : item.title}</b>
          {item.version ? (
            <>
              {' '}
              <span className="t-mono t-xs t-3">{item.version}</span>
            </>
          ) : null}
        </span>
        {key ? <span className="drow__desc">{t(`ferramenta.${key}.descricao`)}</span> : null}
        {item.with.length > 0 ? (
          <span className="drow__desc">{t('junto', { names: item.with.join(', ') })}</span>
        ) : null}
        {item.old || reason ? (
          <span className="drow__meta">
            {item.old ? (
              <span className="tag tag--warn" title={t('antigaDica')}>
                <Icon icon={TriangleAlert} />
                {t('antiga')}
              </span>
            ) : null}
            {reason ? (
              <span className="tag" title={reason.hint ?? undefined}>
                {reason.label}
              </span>
            ) : null}
          </span>
        ) : null}
      </span>
      <span className="drow__end">
        <SourceMark sources={[{ source: item.source }]} />
      </span>
    </div>
  );
}

function KitChoiceRow({
  kit,
  choice,
  onChange,
}: {
  kit: Kit;
  choice: InitialChoice;
  onChange: (choice: InitialChoice) => void;
}) {
  const { t } = useTranslation('modsIniciais');
  const [open, setOpen] = useState(false);
  const listId = useId();
  const chosen = choice.kit?.id === kit.id;
  const selection = new Set(chosen ? choice.kit?.projects : []);
  const extras = Math.max(kit.items.length - 3, 0);
  const preview = kit.items
    .slice(0, 3)
    .map((item) => item.name)
    .join(', ');
  return (
    <div className="drow drow--compact" role="listitem">
      <span className="drow__box">
        <label className="check">
          <input
            type="checkbox"
            checked={chosen}
            aria-label={t('kit.adicionar', { name: kit.name })}
            onChange={(event) => {
              onChange(withKit(choice, event.target.checked ? kit : null));
              if (event.target.checked) setOpen(true);
            }}
          />
        </label>
      </span>
      <span className="drow__main">
        <span className="drow__line">
          <b>{kit.name}</b>
        </span>
        <span className="drow__desc">
          {extras > 0 ? `${preview} ${t('kit.maisQuantos', { count: extras })}. ` : `${preview}. `}
          {kit.description}
        </span>
        <span className="drow__meta">
          <Button
            variant="link"
            icon={open ? ChevronDown : ChevronRight}
            aria-expanded={open}
            aria-controls={listId}
            onClick={() => {
              setOpen(!open);
            }}
          >
            {open ? t('kit.esconder') : t('kit.ver')}
          </Button>
        </span>
        <span id={listId} hidden={!open}>
          {open ? (
            <>
              <KitItems
                kit={kit}
                selection={
                  chosen
                    ? selection
                    : new Set(kit.items.filter((i) => i.checked).map((i) => i.projectId))
                }
                disabled={!chosen}
                onToggle={(projectId) => {
                  onChange(withKitProject(choice, projectId));
                }}
              />
              {chosen ? null : <p className="field__hint">{t('kit.marcarParaEditar')}</p>}
            </>
          ) : null}
        </span>
      </span>
    </div>
  );
}
