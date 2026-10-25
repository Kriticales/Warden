/**
 * Os resultados da página Adicionar nos dois modos do esboço aprovado
 * (`design/bug-01/adicionar.html`, opção 4; ADR-0068):
 *
 * - **Lista:** a tabela densa (linhas de ~40 px; nome e autor, resumo, downloads, atualizado,
 *   fonte e situação). Clicar no título de Nome ordena o que já foi carregado; Downloads e
 *   Atualizado pedem a busca já ordenada às fontes.
 * - **Cartões:** a grade de cartões baixos (3 colunas na janela larga), com o botão Marcar.
 *
 * Os dois mostram os mesmos grupos (no início, Populares e Atualizados recentemente; na busca,
 * os resultados), a mesma seleção e as bibliotecas escondidas: uma linha "N bibliotecas
 * escondidas… Mostrar" na lista, um grupo fechado nos cartões.
 */
import { ArrowDown, ArrowUp, BookOpen, Check, ChevronRight, CircleMinus, Plus } from 'lucide-react';
import { memo, type ReactNode, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import type { SearchResult, SearchSort } from '../../../lib/ipc/bindings';
import { formatInteger } from '../../../lib/format';
import { formatCount, isInPack } from '../common/model';
import { ProjectIcon } from '../common/ProjectIcon';
import { InPackTag, SourceDots, useAgoText } from '../common/text';
import { ManualDownloadTag } from '../curseforge/ManualDownloadNotice';
import { joinNames } from '../dependencies/model';
import {
  type NameSort,
  nextNameSort,
  type ShowOptions,
  shownItems,
  sortByName,
  type ViewMode,
} from './model';

/** Um grupo de resultados ("Populares", "Atualizados recentemente" ou os da busca). */
export interface ResultGroup {
  key: 'populares' | 'atualizados' | 'resultados';
  title: string;
  items: readonly SearchResult[];
  /** Quantos há nas fontes ("17 de 2.481"); nenhum: só a contagem do que aparece. */
  total: number | null;
  /** "Ver todos": a busca completa já ordenada. */
  onSeeAll?: (() => void) | undefined;
  seeAllLabel?: string | undefined;
}

export interface ResultsViewProps {
  view: ViewMode;
  groups: readonly ResultGroup[];
  options: ShowOptions;
  selected: ReadonlyMap<string, SearchResult>;
  currentKey: string | null;
  onToggle: (result: SearchResult) => void;
  onOpen: (result: SearchResult) => void;
  onShowLibraries: () => void;
  sort: SearchSort;
  onSort: (sort: SearchSort) => void;
}

export function ResultsView(props: ResultsViewProps) {
  const [nameSort, setNameSort] = useState<NameSort>(null);
  return props.view === 'cartoes' ? (
    <ResultsCards {...props} />
  ) : (
    <ResultsTable
      {...props}
      nameSort={nameSort}
      onNameSort={() => {
        setNameSort(nextNameSort);
      }}
    />
  );
}

/** "Situação": No pack, Sem versão, Download manual ou Biblioteca (uma só). */
function StateTag({ result, inPack }: { result: SearchResult; inPack: boolean }) {
  const { t } = useTranslation('adicionar');
  if (inPack) return <InPackTag />;
  if (!result.compatible) {
    return (
      <span className="tag tag--plain t-3" title={t('resultado.semVersaoDica')}>
        <Icon icon={CircleMinus} />
        {t('resultado.semVersao')}
      </span>
    );
  }
  if (result.manualDownload) return <ManualDownloadTag short />;
  if (result.isLibrary) {
    return (
      <span className="tag tag--lib" title={t('resultado.bibliotecaDica')}>
        <Icon icon={BookOpen} />
        {t('resultado.biblioteca')}
      </span>
    );
  }
  return null;
}

/** "17 de 2.481" (no início) ou só "17" (na busca). */
function useGroupCount() {
  const { t } = useTranslation('adicionar');
  return (shown: number, total: number | null) =>
    total === null
      ? formatInteger(shown)
      : t('resultado.deTotal', { count: shown, total: formatInteger(Math.max(total, shown)) });
}

/* ---------------------------------- Lista ---------------------------------- */

interface TableProps extends ResultsViewProps {
  nameSort: NameSort;
  onNameSort: () => void;
}

function ResultsTable({
  groups,
  options,
  selected,
  currentKey,
  onToggle,
  onOpen,
  onShowLibraries,
  sort,
  onSort,
  nameSort,
  onNameSort,
}: TableProps) {
  const { t } = useTranslation('adicionar');
  const groupCount = useGroupCount();
  const serverSort = (by: SearchSort) => {
    onSort(sort === by ? 'relevance' : by);
  };
  return (
    <div className="tbl" role="table" aria-label={t('resultado.tabela')}>
      <div className="trow thead" role="row">
        <span role="columnheader">
          <span className="sr-only">{t('resultado.marcarDica')}</span>
        </span>
        <SortHeader
          label={t('colunas.nome')}
          sort={nameSort === 'asc' ? 'ascending' : nameSort === 'desc' ? 'descending' : null}
          onClick={onNameSort}
        />
        <span role="columnheader" className="tsum">
          {t('colunas.resumo')}
        </span>
        <SortHeader
          className="r"
          label={t('colunas.downloads')}
          sort={sort === 'downloads' ? 'descending' : null}
          onClick={() => {
            serverSort('downloads');
          }}
        />
        <SortHeader
          className="tupd"
          label={t('colunas.atualizado')}
          sort={sort === 'updated' ? 'descending' : null}
          onClick={() => {
            serverSort('updated');
          }}
        />
        <span role="columnheader">{t('colunas.fonte')}</span>
        <span role="columnheader">{t('colunas.situacao')}</span>
      </div>
      {groups.map((group) => {
        const shown = shownItems(group.items, options);
        const items = sortByName(shown.items, nameSort);
        return (
          <div key={group.key} role="rowgroup" aria-label={group.title}>
            <div className="trow tgrp" role="row">
              <span role="cell" className="tgrp__cell">
                <h2 className="tgrp__t">{group.title}</h2>
                <span className="tgrp__n">{groupCount(items.length, group.total)}</span>
                {group.onSeeAll ? (
                  <Button variant="link" aria-label={group.seeAllLabel} onClick={group.onSeeAll}>
                    {t('inicio.verTodos', { ns: 'descoberta' })}
                  </Button>
                ) : null}
              </span>
            </div>
            {items.length === 0 ? (
              <div className="trow tempty" role="row">
                <span role="cell">{t('resultado.nadaComFiltros')}</span>
              </div>
            ) : (
              items.map((result) => (
                <TableRow
                  key={result.key}
                  result={result}
                  inPack={isInPack(result, options.inventoryKeys)}
                  selected={selected.has(result.key)}
                  current={currentKey === result.key}
                  onToggle={onToggle}
                  onOpen={onOpen}
                />
              ))
            )}
            {shown.hiddenLibraries.length > 0 ? (
              <div className="trow tlib" role="row">
                <span role="cell" className="tlib__cell">
                  <Icon icon={BookOpen} />
                  <span className="tlib__t">
                    <b>{t('bibliotecas.escondidas', { count: shown.hiddenLibraries.length })}</b>{' '}
                    <span>{t('bibliotecas.porque')}</span>{' '}
                    {joinNames(shown.hiddenLibraries.map((item) => item.title))}
                  </span>
                  <Button size="sm" onClick={onShowLibraries}>
                    {t('bibliotecas.mostrar')}
                  </Button>
                </span>
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function SortHeader({
  label,
  sort,
  onClick,
  className,
}: {
  label: string;
  sort: 'ascending' | 'descending' | null;
  onClick: () => void;
  className?: string;
}) {
  const { t } = useTranslation('adicionar');
  return (
    <span role="columnheader" aria-sort={sort ?? undefined} className={className}>
      <button
        type="button"
        className="thead__sort"
        title={t('colunas.ordenarPor', { coluna: label })}
        onClick={onClick}
      >
        {label}
        {sort ? <Icon icon={sort === 'descending' ? ArrowDown : ArrowUp} size="sm" /> : null}
      </button>
    </span>
  );
}

interface ItemProps {
  result: SearchResult;
  inPack: boolean;
  selected: boolean;
  current: boolean;
  onToggle: (result: SearchResult) => void;
  onOpen: (result: SearchResult) => void;
}

const TableRow = memo(function TableRow({
  result,
  inPack,
  selected,
  current,
  onToggle,
  onOpen,
}: ItemProps) {
  const { t } = useTranslation('adicionar');
  const agoText = useAgoText();
  return (
    <div
      className={cn(
        'trow',
        selected && 'is-sel',
        inPack && 'is-in',
        !result.compatible && 'is-nover',
        current && 'is-open',
      )}
      role="row"
    >
      <span role="cell">
        {inPack ? null : (
          <label className="check" title={t('resultado.marcarDica')}>
            <input
              type="checkbox"
              checked={selected}
              disabled={!result.compatible}
              aria-label={t('resultado.marcar', { name: result.title })}
              onChange={() => {
                onToggle(result);
              }}
            />
          </label>
        )}
      </span>
      <span role="cell" className="tname">
        <ProjectIcon url={result.iconUrl} name={result.title} className="tname__icon" size="sm" />
        <button
          type="button"
          className="tname__n"
          title={t('resultado.abrirDica')}
          aria-current={current ? 'true' : undefined}
          onClick={() => {
            onOpen(result);
          }}
        >
          {result.title}
        </button>
        {result.author ? <span className="tname__a">{result.author}</span> : null}
      </span>
      <span role="cell" className="tsum" title={result.summary}>
        {result.summary}
      </span>
      <span role="cell" className="tnum r">
        {formatCount(result.downloads)}
      </span>
      <span role="cell" className="tupd">
        {agoText(result.updated)}
      </span>
      <span role="cell">
        <SourceDots sources={result.sources} />
      </span>
      <span role="cell">
        <StateTag result={result} inPack={inPack} />
      </span>
    </div>
  );
});

/* --------------------------------- Cartões --------------------------------- */

const SORTS: readonly SearchSort[] = ['relevance', 'downloads', 'updated', 'newest'];

function ResultsCards({
  groups,
  options,
  selected,
  currentKey,
  onToggle,
  onOpen,
  sort,
  onSort,
}: ResultsViewProps) {
  const { t } = useTranslation('adicionar');
  const groupCount = useGroupCount();
  const id = useId();
  return (
    <div className="cards">
      <div className="cards__bar">
        <label className="t-sm t-3" htmlFor={`${id}-sort`}>
          {t('filtros.ordenar')}
        </label>
        <select
          id={`${id}-sort`}
          className="select cards__sort"
          value={sort}
          onChange={(event) => {
            onSort(SORTS.find((value) => value === event.target.value) ?? 'relevance');
          }}
        >
          {SORTS.map((value) => (
            <option key={value} value={value}>
              {t(`filtros.ordem.${value}`)}
            </option>
          ))}
        </select>
      </div>
      {groups.map((group) => {
        const shown = shownItems(group.items, options);
        return (
          <section key={group.key} className="grp" aria-labelledby={`${id}-${group.key}`}>
            <div className="grp__head">
              <h2 className="tgrp__t" id={`${id}-${group.key}`}>
                {group.title}
              </h2>
              <span className="tgrp__n">{groupCount(shown.items.length, group.total)}</span>
              {group.onSeeAll ? (
                <Button variant="link" aria-label={group.seeAllLabel} onClick={group.onSeeAll}>
                  {t('inicio.verTodos', { ns: 'descoberta' })}
                </Button>
              ) : null}
            </div>
            {shown.items.length > 0 ? (
              <ul className="cgrid" aria-label={group.title}>
                {shown.items.map((result) => (
                  <Card
                    key={result.key}
                    result={result}
                    inPack={isInPack(result, options.inventoryKeys)}
                    selected={selected.has(result.key)}
                    current={currentKey === result.key}
                    onToggle={onToggle}
                    onOpen={onOpen}
                  />
                ))}
              </ul>
            ) : (
              <p className="tempty">{t('resultado.nadaComFiltros')}</p>
            )}
            {shown.hiddenLibraries.length > 0 ? (
              <LibraryGroup
                title={
                  group.key === 'populares'
                    ? t('bibliotecas.grupoPopulares', { count: shown.hiddenLibraries.length })
                    : t('bibliotecas.grupo', { count: shown.hiddenLibraries.length })
                }
              >
                <ul className="cgrid">
                  {shown.hiddenLibraries.map((result) => (
                    <Card
                      key={result.key}
                      result={result}
                      inPack={isInPack(result, options.inventoryKeys)}
                      selected={selected.has(result.key)}
                      current={currentKey === result.key}
                      onToggle={onToggle}
                      onOpen={onOpen}
                    />
                  ))}
                </ul>
              </LibraryGroup>
            ) : null}
          </section>
        );
      })}
    </div>
  );
}

function LibraryGroup({ title, children }: { title: string; children: ReactNode }) {
  const { t } = useTranslation('adicionar');
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <div className={cn('libgrp', open && 'is-open')}>
      <button
        type="button"
        className="libgrp__sum"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => {
          setOpen((now) => !now);
        }}
      >
        <Icon icon={BookOpen} />
        <b>{title}</b>
        <span className="t-3 libgrp__hint">{t('bibliotecas.grupoDica')}</span>
        <Icon icon={ChevronRight} size="sm" className="libgrp__chev" />
      </button>
      <div className="libgrp__body" id={id} hidden={!open}>
        {open ? children : null}
      </div>
    </div>
  );
}

const Card = memo(function Card({
  result,
  inPack,
  selected,
  current,
  onToggle,
  onOpen,
}: ItemProps) {
  const { t } = useTranslation('adicionar');
  const agoText = useAgoText();
  return (
    <li className={cn('mcard', selected && 'is-sel', current && 'is-open', inPack && 'is-in')}>
      <ProjectIcon url={result.iconUrl} name={result.title} className="mcard__icon" size="lg" />
      <div className="mcard__top">
        <button
          type="button"
          className="mcard__n"
          title={t('resultado.abrirDica')}
          aria-current={current ? 'true' : undefined}
          onClick={() => {
            onOpen(result);
          }}
        >
          {result.title}
        </button>
        {inPack ? null : (
          <Button
            size="sm"
            className="mcard__mark"
            icon={selected ? Check : Plus}
            aria-pressed={selected}
            aria-label={t('resultado.marcar', { name: result.title })}
            disabled={!result.compatible}
            onClick={() => {
              onToggle(result);
            }}
          >
            {selected ? t('resultado.marcadoBotao') : t('resultado.marcarBotao')}
          </Button>
        )}
      </div>
      <span className="mcard__by">
        {result.author ? t('resultado.por', { author: result.author }) : null}
      </span>
      <p className="mcard__sum">{result.summary}</p>
      <div className="mcard__foot">
        <SourceDots sources={result.sources} />
        <span className="tnum">{formatCount(result.downloads)}</span>
        <span>{agoText(result.updated)}</span>
        <StateTag result={result} inPack={inPack} />
      </div>
    </li>
  );
});
