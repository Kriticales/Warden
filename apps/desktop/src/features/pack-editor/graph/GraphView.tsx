/**
 * Modo Grafo da seção Mods (SPEC T06, "Ver como: Lista · Grafo"; CA-T06-07; D-11): o seletor
 * do mod no centro (com busca) e da profundidade, o grafo focado, a explicação em palavras, a
 * lista das ligações, as bibliotecas sem uso e "Ver o pack inteiro". Os nós abrem os detalhes
 * (T07) no mesmo painel lateral da lista, que a página de Mods mantém.
 */
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type { InventoryItem, PackId } from '../../../lib/ipc/bindings';
import { useGraphFocus, useOrphans, type Depth } from './api';
import { FocusedGraph } from './FocusedGraph';
import { EdgeList, InWords, OrphansNote } from './GraphText';
import { centerOptions } from './model';
import { ViewAs, type ModsView } from './ViewAs';
import { WholeGraphDialog } from './WholeGraphDialog';
import './graph.css';

export interface GraphViewProps {
  packId: PackId;
  items: readonly InventoryItem[];
  /** Caminho do mod no centro (da rota), ou `null` para o primeiro mod. */
  center: string | null;
  onCenter: (path: string) => void;
  onView: (view: ModsView) => void;
  onOpenItem: (path: string) => void;
}

/** Só mods legíveis entram no grafo (o mesmo critério do backend). */
function graphMods(items: readonly InventoryItem[]): InventoryItem[] {
  return items.filter((item) => item.kind === 'mod' && item.state === 'ok');
}

export function GraphView({ packId, items, center, onCenter, onView, onOpenItem }: GraphViewProps) {
  const { t } = useTranslation('grafoDesenho');
  const mods = graphMods(items);
  const [depth, setDepth] = useState<Depth>(1);
  const [search, setSearch] = useState('');
  const [whole, setWhole] = useState(false);
  const fallback = centerOptions(mods, '')[0]?.path ?? null;
  const chosen = center !== null && mods.some((item) => item.path === center) ? center : fallback;
  const focus = useGraphFocus(packId, chosen, depth);
  const orphans = useOrphans(packId);
  const options = centerOptions(mods, search);
  const ids = { center: useId(), search: useId() };

  const toolbar = (
    <div className="toolbar grafo-toolbar">
      <ViewAs view="grafo" onChange={onView} />
      {mods.length > 0 ? (
        <>
          <div className="inputwrap">
            <input
              id={ids.search}
              className="input"
              type="search"
              value={search}
              placeholder={t('ferramentas.buscarCentro')}
              aria-label={t('ferramentas.buscarCentro')}
              aria-controls={ids.center}
              onChange={(event) => {
                setSearch(event.target.value);
              }}
            />
          </div>
          <label className="t-xs t-3" htmlFor={ids.center}>
            {t('ferramentas.centro')}
          </label>
          <select
            id={ids.center}
            className="select"
            value={options.some((item) => item.path === chosen) ? (chosen ?? '') : ''}
            onChange={(event) => {
              if (event.target.value !== '') onCenter(event.target.value);
            }}
          >
            {options.some((item) => item.path === chosen) ? null : (
              <option value="" disabled>
                {options.length === 0 ? t('ferramentas.semResultado') : t('ferramentas.centro')}
              </option>
            )}
            {options.map((item) => (
              <option key={item.path} value={item.path}>
                {item.name}
              </option>
            ))}
          </select>
          <select
            className="select"
            aria-label={t('ferramentas.profundidade')}
            value={depth}
            onChange={(event) => {
              setDepth(event.target.value === '2' ? 2 : 1);
            }}
          >
            {([1, 2] as const).map((level) => (
              <option key={level} value={level}>
                {t('ferramentas.nivel', { count: level })}
              </option>
            ))}
          </select>
        </>
      ) : null}
    </div>
  );

  if (mods.length === 0) {
    return (
      <>
        {toolbar}
        <EmptyState compact title={t('semMods.titulo')} text={t('semMods.texto')} />
      </>
    );
  }

  return (
    <>
      {toolbar}
      {focus.isError ? (
        <ErrorPanel
          error={focus.error}
          onRetry={() => {
            void focus.refetch();
          }}
        />
      ) : !focus.data ? (
        <LoadingState inline label={t('carregando')} />
      ) : (
        <div className="stack">
          <FocusedGraph report={focus.data} onOpen={onOpenItem} />
          <InWords packId={packId} report={focus.data} />
          <EdgeList report={focus.data} />
          <OrphansNote orphans={orphans.data ?? []} onOpen={onOpenItem} />
          <p className="t-xs t-3">
            {t('rodape')}{' '}
            <Button
              variant="link"
              onClick={() => {
                setWhole(true);
              }}
            >
              {t('inteiro.abrir')}
            </Button>
          </p>
        </div>
      )}
      <WholeGraphDialog
        packId={packId}
        open={whole}
        onClose={() => {
          setWhole(false);
        }}
        onOpen={(path) => {
          setWhole(false);
          onOpenItem(path);
        }}
      />
    </>
  );
}
