/**
 * Seção Mods (SPEC T06; protótipo `modsContent`): uma lista única de mods, resource packs e
 * shaders, agrupada por tipo, com busca, filtros (caixas de seleção, nunca abas), seleção
 * múltipla com Alterar lado e Remover, lado editável na linha e os detalhes num painel lateral
 * (T07) sem sair da lista. Um arquivo ruim nunca derruba a lista.
 *
 * Adicionar (P1-09), Verificar atualizações (P1-12) e o modo Grafo (D-07/P1) chegam com as
 * tarefas donas; até lá os botões ficam indisponíveis, com o motivo na dica.
 */
import { Plus, RefreshCw, Search } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Skeleton } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type {
  Inventory,
  InventoryItem,
  ItemKind,
  ItemSide,
  ItemSource,
  PackId,
  SideChoice,
} from '../../../lib/ipc/bindings';
import { usePack } from '../../packs/api';
import { loaderName } from '../../packs/lib/pack-list';
import { useIncludeOutside, useInventory, useOpenItemFile, useSetSide } from '../api';
import { ItemDrawer } from '../details/ItemDrawer';
import { ItemGroupTable } from './ItemGroupTable';
import {
  countByKind,
  groupByKind,
  hasFilters,
  matches,
  NO_FILTERS,
  selectedItems,
  sortItems,
  type ModsFilters,
} from './model';
import { ModRow } from './ModRow';
import { RemoveDialog } from './RemoveDialog';
import { SelectionBar } from './SelectionBar';

export interface ModsPageProps {
  packId: PackId;
  /** Caminho do item com os detalhes abertos (da busca da rota), ou `null`. */
  openItem: string | null;
  onOpenItem: (path: string | null) => void;
}

export function ModsPage({ packId, openItem, onOpenItem }: ModsPageProps) {
  const { t } = useTranslation('editor');
  const inventory = useInventory(packId);

  if (inventory.isPending) {
    return (
      <>
        <ModsHead summary={t('mods.carregando')} />
        <div className="tablewrap" aria-busy="true">
          <div className="stack-2 modlist__skeleton">
            {Array.from({ length: 8 }, (_, index) => (
              <Skeleton key={index} />
            ))}
          </div>
        </div>
      </>
    );
  }
  if (inventory.isError) {
    return (
      <>
        <ModsHead summary={null} />
        <ErrorPanel
          error={inventory.error}
          onRetry={() => {
            void inventory.refetch();
          }}
        />
      </>
    );
  }
  return (
    <ModsList
      packId={packId}
      inventory={inventory.data}
      openItem={openItem}
      onOpenItem={onOpenItem}
    />
  );
}

/** Título da seção, o resumo e os botões Verificar atualizações e Adicionar. */
function ModsHead({ summary }: { summary: string | null }) {
  const { t } = useTranslation('editor');
  return (
    <PageHead
      title={t('mods.titulo')}
      sub={summary}
      actions={
        <>
          <UnavailableButton icon={RefreshCw} reason={t('mods.verificarIndisponivel')}>
            {t('mods.verificarAtualizacoes')}
          </UnavailableButton>
          <UnavailableButton icon={Plus} primary reason={t('mods.adicionarIndisponivel')}>
            {t('mods.adicionar')}
          </UnavailableButton>
        </>
      }
    />
  );
}

/** Botão de uma função que ainda não chegou: visível, indisponível e com o motivo na dica. */
function UnavailableButton({
  icon,
  primary = false,
  reason,
  children,
}: {
  icon: typeof Plus;
  primary?: boolean;
  reason: string;
  children: string;
}) {
  return (
    <Tooltip content={reason}>
      <Button
        icon={icon}
        variant={primary ? 'primary' : 'secondary'}
        aria-disabled="true"
        onClick={(event) => {
          event.preventDefault();
        }}
      >
        {children}
      </Button>
    </Tooltip>
  );
}

interface ModsListProps {
  packId: PackId;
  inventory: Inventory;
  openItem: string | null;
  onOpenItem: (path: string | null) => void;
}

function ModsList({ packId, inventory, openItem, onOpenItem }: ModsListProps) {
  const { t } = useTranslation('editor');
  const pack = usePack(packId);
  const [filters, setFilters] = useState<ModsFilters>(NO_FILTERS);
  const [sort, setSort] = useState<'asc' | 'desc'>('asc');
  const [collapsed, setCollapsed] = useState<ReadonlySet<ItemKind>>(new Set());
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [removing, setRemoving] = useState<InventoryItem[] | null>(null);
  const [errorOf, setErrorOf] = useState<InventoryItem | null>(null);
  const setSide = useSetSide(packId);
  const include = useIncludeOutside(packId);
  const openFile = useOpenItemFile(packId);

  const items = inventory.items;
  const counts = useMemo(() => countByKind(items), [items]);
  const groups = useMemo(
    () =>
      groupByKind(
        sortItems(
          items.filter((item) => matches(item, filters)),
          sort,
        ),
      ),
    [items, filters, sort],
  );
  const invalid = items.filter((item) => item.state === 'invalid').length;
  const outside = items.filter((item) => item.state === 'outsideIndex').length;
  const chosen = selectedItems(items, selected);

  const toggle = useCallback((path: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  // O resultado (sucesso ou falha) aparece no aviso da operação ("Concluída: Alterar lado"),
  // que o app mostra para toda escrita no pack (T22).
  const changeSide = useCallback(
    (paths: string[], side: SideChoice) => {
      setSide.mutate({ paths, side });
    },
    [setSide],
  );
  const onRowSide = useCallback(
    (path: string, side: SideChoice) => {
      changeSide([path], side);
    },
    [changeSide],
  );
  const onShowError = useCallback((item: InventoryItem) => {
    setErrorOf(item);
  }, []);
  const onOpenFile = useCallback(
    (path: string) => {
      openFile.mutate(path, {
        onError: () => {
          showToast({ kind: 'danger', title: t('mods.acoesInvalido.abrirFalhou') });
        },
      });
    },
    [openFile, t],
  );

  const detailItem = openItem ? items.find((item) => item.path === openItem) : undefined;
  const summary = summaryText(t, counts);

  if (items.length === 0 && !inventory.indexError) {
    const data = pack.data;
    return (
      <>
        <ModsHead summary={summary} />
        <EmptyState
          glyph="plus"
          title={t('mods.vazio.titulo')}
          text={t('mods.vazio.texto', {
            name: data?.name ?? '',
            minecraft: data?.minecraft ?? '?',
            loader: data?.loader
              ? t('mods.vazio.comLoader', {
                  loader: `${loaderName(data.loader)} ${data.loaderVersion ?? ''}`.trim(),
                })
              : '',
          })}
          actions={
            <UnavailableButton icon={Plus} primary reason={t('mods.adicionarIndisponivel')}>
              {t('mods.vazio.acao')}
            </UnavailableButton>
          }
        />
      </>
    );
  }

  return (
    <>
      <ModsHead summary={summary} />
      {inventory.indexError ? (
        <Alert kind="danger" className="mods-banner" title={t('mods.indiceIlegivel')}>
          <div className="alert__text">{t('mods.indiceIlegivelTexto')}</div>
          <pre className="errpanel__pre">{inventory.indexError}</pre>
        </Alert>
      ) : null}
      {invalid > 0 ? (
        <Alert
          kind="danger"
          className="mods-banner"
          title={t('mods.invalidos', { count: invalid })}
          actions={
            <Button
              size="sm"
              onClick={() => {
                setFilters({ ...NO_FILTERS, show: 'problems' });
              }}
            >
              {t('mods.verComProblemas', { count: invalid + outside })}
            </Button>
          }
        >
          <div className="alert__text">{t('mods.invalidosTexto')}</div>
        </Alert>
      ) : null}
      {outside > 0 ? (
        <Alert
          kind="warn"
          className="mods-banner"
          title={t('mods.fora', { count: outside })}
          actions={
            <Button
              size="sm"
              loading={include.isPending}
              onClick={() => {
                include.mutate();
              }}
            >
              {include.isPending ? t('mods.incluindo') : t('mods.incluir')}
            </Button>
          }
        >
          <div className="alert__text">{t('mods.foraTexto')}</div>
          {include.isError ? <ErrorPanel compact error={include.error} /> : null}
        </Alert>
      ) : null}
      <Toolbar filters={filters} onChange={setFilters} items={items} />
      {chosen.length > 0 ? (
        <SelectionBar
          items={chosen}
          busy={setSide.isPending}
          onSide={(side) => {
            changeSide(
              chosen.filter((item) => item.sideEditable).map((item) => item.path),
              side,
            );
          }}
          onRemove={() => {
            setRemoving(chosen);
          }}
          onClear={() => {
            setSelected(new Set());
          }}
        />
      ) : null}
      {groups.length === 0 ? (
        <EmptyState
          compact
          title={t('mods.filtros.semResultado')}
          text={t('mods.filtros.semResultadoTexto')}
          actions={
            <Button
              onClick={() => {
                setFilters(NO_FILTERS);
              }}
            >
              {t('mods.filtros.limpar')}
            </Button>
          }
        />
      ) : (
        groups.map((group) => {
          const inGroup = group.items.filter((item) => selected.has(item.path)).length;
          return (
            <ItemGroupTable
              key={group.key}
              kind={group.key}
              items={group.items}
              collapsed={collapsed.has(group.key)}
              onToggleCollapsed={() => {
                setCollapsed((current) => {
                  const next = new Set(current);
                  if (next.has(group.key)) next.delete(group.key);
                  else next.add(group.key);
                  return next;
                });
              }}
              allSelected={inGroup === 0 ? false : inGroup === group.items.length ? true : 'some'}
              onSelectAll={(value) => {
                setSelected((current) => {
                  const next = new Set(current);
                  for (const item of group.items) {
                    if (value) next.add(item.path);
                    else next.delete(item.path);
                  }
                  return next;
                });
              }}
              sortDirection={sort}
              onSort={() => {
                setSort((current) => (current === 'asc' ? 'desc' : 'asc'));
              }}
              renderRow={(item, rowIndex) => (
                <ModRow
                  key={item.path}
                  item={item}
                  rowIndex={rowIndex}
                  selected={selected.has(item.path)}
                  sideBusy={setSide.isPending}
                  onToggle={toggle}
                  onOpen={onOpenItem}
                  onSide={onRowSide}
                  onShowError={onShowError}
                  onOpenFile={onOpenFile}
                />
              )}
            />
          );
        })
      )}
      <ItemDrawer
        packId={packId}
        item={detailItem ?? null}
        onClose={() => {
          onOpenItem(null);
        }}
        onRemove={(item) => {
          setRemoving([item]);
        }}
      />
      <RemoveDialog
        packId={packId}
        items={removing}
        onClose={() => {
          setRemoving(null);
        }}
        onRemoved={(paths) => {
          setSelected((current) => new Set([...current].filter((path) => !paths.includes(path))));
          if (openItem && paths.includes(openItem)) onOpenItem(null);
        }}
      />
      <Dialog
        open={errorOf !== null}
        onOpenChange={(open) => {
          if (!open) setErrorOf(null);
        }}
      >
        {errorOf ? (
          <DialogContent
            title={t('mods.acoesInvalido.erroTitulo', { file: errorOf.path })}
            footer={
              <Button
                onClick={() => {
                  onOpenFile(errorOf.path);
                }}
              >
                {t('mods.acoesInvalido.abrirEditor')}
              </Button>
            }
          >
            <pre className="errpanel__pre">{errorOf.error ?? ''}</pre>
          </DialogContent>
        ) : null}
      </Dialog>
    </>
  );
}

type EditorT = ReturnType<typeof useTranslation<'editor'>>['t'];

/** "128 itens: 124 mods, 3 resource packs e 1 shader." */
function summaryText(t: EditorT, counts: Record<ItemKind, number>): string {
  const parts: string[] = [];
  if (counts.mod > 0) parts.push(t('mods.resumoMods', { count: counts.mod }));
  if (counts.resourcePack > 0) {
    parts.push(t('mods.resumoResourcePacks', { count: counts.resourcePack }));
  }
  if (counts.shader > 0) parts.push(t('mods.resumoShaders', { count: counts.shader }));
  if (counts.other > 0) parts.push(t('mods.resumoOutros', { count: counts.other }));
  const total = counts.mod + counts.resourcePack + counts.shader + counts.other;
  const detail =
    parts.length <= 1
      ? (parts[0] ?? '')
      : `${parts.slice(0, -1).join(', ')}${t('mods.e')}${parts[parts.length - 1] ?? ''}`;
  return t('mods.resumo', { count: total, detalhe: detail });
}

const KINDS: readonly ItemKind[] = ['mod', 'resourcePack', 'shader', 'other'];
const SOURCES: readonly ItemSource[] = ['modrinth', 'curseforge', 'url', 'local'];
const SIDES: readonly ItemSide[] = ['both', 'client', 'server'];
const SHOW: readonly ModsFilters['show'][] = ['all', 'problems', 'pinned', 'optional'];

/** Busca e filtros (SPEC T06: caixas de seleção, nunca abas) e "Ver como: Lista · Grafo". */
function Toolbar({
  filters,
  onChange,
  items,
}: {
  filters: ModsFilters;
  onChange: (filters: ModsFilters) => void;
  items: readonly InventoryItem[];
}) {
  const { t } = useTranslation('editor');
  const problems = items.filter((item) => item.state !== 'ok').length;
  const pinned = items.filter((item) => item.pinned).length;
  const optional = items.filter((item) => item.optional).length;
  return (
    <div className="toolbar" role="search">
      <span className="row">
        <span className="t-xs t-3" id="ver-como">
          {t('mods.filtros.verComo')}
        </span>
        <span className="segmented" role="radiogroup" aria-labelledby="ver-como">
          <button type="button" role="radio" aria-checked="true" tabIndex={0}>
            {t('mods.filtros.lista')}
          </button>
          <Tooltip content={t('mods.filtros.grafoIndisponivel')}>
            <button
              type="button"
              role="radio"
              aria-checked="false"
              aria-disabled="true"
              tabIndex={-1}
            >
              {t('mods.filtros.grafo')}
            </button>
          </Tooltip>
        </span>
      </span>
      <div className="inputwrap">
        <Icon icon={Search} />
        <input
          className="input"
          type="search"
          value={filters.search}
          placeholder={t('mods.filtros.buscar')}
          aria-label={t('mods.filtros.buscar')}
          onChange={(event) => {
            onChange({ ...filters, search: event.target.value });
          }}
        />
      </div>
      <select
        className="select"
        aria-label={t('mods.filtros.tipo')}
        value={filters.kind ?? ''}
        onChange={(event) => {
          onChange({ ...filters, kind: KINDS.find((k) => k === event.target.value) ?? null });
        }}
      >
        <option value="">{t('mods.filtros.tipoTodos')}</option>
        {KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {t(`mods.grupos.${kind}`)}
          </option>
        ))}
      </select>
      <select
        className="select"
        aria-label={t('mods.filtros.fonte')}
        value={filters.source ?? ''}
        onChange={(event) => {
          onChange({ ...filters, source: SOURCES.find((s) => s === event.target.value) ?? null });
        }}
      >
        <option value="">{t('mods.filtros.fonteTodas')}</option>
        {SOURCES.map((source) => (
          <option key={source} value={source}>
            {t(`mods.fonte.${source}`)}
          </option>
        ))}
      </select>
      <select
        className="select"
        aria-label={t('mods.filtros.lado')}
        value={filters.side ?? ''}
        onChange={(event) => {
          onChange({ ...filters, side: SIDES.find((s) => s === event.target.value) ?? null });
        }}
      >
        <option value="">{t('mods.filtros.ladoTodos')}</option>
        {SIDES.map((side) => (
          <option key={side} value={side}>
            {t(`mods.lado.${side}`)}
          </option>
        ))}
      </select>
      <select
        className="select"
        aria-label={t('mods.filtros.mostrar')}
        value={filters.show}
        onChange={(event) => {
          const show = SHOW.find((value) => value === event.target.value);
          onChange({ ...filters, show: show ?? NO_FILTERS.show });
        }}
      >
        {SHOW.map((show) => (
          <option key={show} value={show}>
            {show === 'all'
              ? t('mods.filtros.mostrarTudo')
              : show === 'problems'
                ? t('mods.filtros.mostrarProblemas', { count: problems })
                : show === 'pinned'
                  ? t('mods.filtros.mostrarFixados', { count: pinned })
                  : t('mods.filtros.mostrarOpcionais', { count: optional })}
          </option>
        ))}
      </select>
      {hasFilters(filters) ? (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            onChange(NO_FILTERS);
          }}
        >
          {t('mods.filtros.limpar')}
        </Button>
      ) : null}
    </div>
  );
}
