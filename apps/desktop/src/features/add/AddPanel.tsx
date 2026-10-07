/**
 * Página Adicionar (SPEC T08; ESTRUTURA N6; protótipo `adicionar-busca`): em tela cheia, com o
 * menu do pack recolhido (a rota declara `packMenu: 'compact'`) e o cabeçalho do pack visível.
 * Sem abas: seletor **Tipo**, um campo único de busca e **Escolher arquivo do computador…**
 * (P1-11). Três colunas: filtros, resultados (com caixa de seleção e a barra "N selecionados ·
 * Adicionar N ao pack") e pré-visualização. Adicionar abre sempre **um** diálogo de dependências
 * (T09) para o conjunto; depois de gravar, a busca continua aberta e os itens passam a mostrar
 * "Já no pack".
 *
 * O início com populares, atualizados e categorias (campo vazio) é da P1-16; aqui o campo vazio
 * mostra a busca sem texto, já filtrada para o pack. Link colado no campo é da P1-11.
 */
import { Link } from '@tanstack/react-router';
import { ArrowLeft, FilePlus, Plus, RefreshCw, Search } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Skeleton } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button, buttonVariants } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Tooltip } from '../../components/ui/tooltip';
import { cn } from '../../lib/cn';
import type {
  AddResult,
  PackId,
  ProjectKind,
  SearchResult,
  SourceWarning,
} from '../../lib/ipc/bindings';
import { useInventory } from '../pack-editor/api';
import { usePack } from '../packs/api';
import { useSearch } from './common/api';
import { FiltersColumn, NO_FILTERS, type UserFilters } from './common/FiltersColumn';
import { isInPack, knownSources, mergePages, pageWarnings, sourceLabel } from './common/model';
import { Preview } from './common/Preview';
import { ResultRow } from './common/ResultRow';
import { type PackTarget, useTargetText } from './common/text';
import { DependenciesDialog, type DependenciesRequest } from './dependencies/DependenciesDialog';
import { preferredRef } from './modrinth/source';
import './common/add.css';

export const PROJECT_KINDS: readonly ProjectKind[] = ['mod', 'resourcePack', 'shader'];

/** Espera entre a última tecla e a busca. */
const TYPING_DELAY_MS = 300;

export interface AddPanelProps {
  packId: PackId;
  kind: ProjectKind;
  onKindChange: (kind: ProjectKind) => void;
}

function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebounced(value);
    }, delay);
    return () => {
      clearTimeout(timer);
    };
  }, [value, delay]);
  return debounced;
}

export function AddPanel({ packId, kind, onKindChange }: AddPanelProps) {
  const { t } = useTranslation('adicionar');
  const pack = usePack(packId);
  const inventory = useInventory(packId);
  const [text, setText] = useState('');
  const query = useDebounced(text.trim(), TYPING_DELAY_MS);
  const [filters, setFilters] = useState<UserFilters>(NO_FILTERS);
  const search = useSearch(packId, { query, kind, ...filters });
  const [selected, setSelected] = useState<ReadonlyMap<string, SearchResult>>(new Map());
  const [current, setCurrent] = useState<SearchResult | null>(null);
  const [request, setRequest] = useState<DependenciesRequest | null>(null);
  const [added, setAdded] = useState(0);
  const id = useId();

  const target: PackTarget = {
    minecraft: pack.data?.minecraft ?? '',
    loader: pack.data?.loader ?? null,
  };
  const pages = useMemo(() => search.data?.pages ?? [], [search.data]);
  const results = useMemo(() => mergePages(pages), [pages]);
  const warnings = useMemo(() => pageWarnings(pages), [pages]);
  const sources = useMemo(() => knownSources(pages), [pages]);
  const inventoryKeys = useMemo(
    () => new Set((inventory.data?.items ?? []).map((item) => item.key)),
    [inventory.data],
  );

  const toggle = useCallback((result: SearchResult) => {
    setSelected((now) => {
      const next = new Map(now);
      if (next.has(result.key)) next.delete(result.key);
      else next.set(result.key, result);
      return next;
    });
  }, []);

  const openDialog = (items: readonly SearchResult[]) => {
    const choices = items.flatMap((result) => {
      const ref = preferredRef(result.sources);
      return ref ? [{ source: ref.source, projectId: ref.projectId, versionId: null }] : [];
    });
    setRequest({ choices, titles: items.map((result) => result.title) });
  };

  const onAdded = (result: AddResult) => {
    setAdded((count) => count + result.added.length);
    setSelected(new Map());
    setRequest(null);
  };

  const loading = search.isPending || (search.isFetching && !search.isFetchingNextPage);

  return (
    <div className="add-page">
      <PageHead
        title={t('titulo')}
        back={
          <Link
            to="/packs/$packId/mods"
            params={{ packId }}
            className={buttonVariants({ variant: 'ghost', size: 'sm' })}
          >
            <Icon icon={ArrowLeft} />
            <span>{added > 0 ? t('voltarAdicionados', { count: added }) : t('voltar')}</span>
          </Link>
        }
      />
      <div className="add-top" role="search">
        <select
          className="select"
          aria-label={t('tipo.rotulo')}
          value={kind}
          onChange={(event) => {
            const next = PROJECT_KINDS.find((value) => value === event.target.value);
            if (next) {
              setSelected(new Map());
              setCurrent(null);
              onKindChange(next);
            }
          }}
        >
          {PROJECT_KINDS.map((value) => (
            <option key={value} value={value}>
              {t(`tipo.${value}`)}
            </option>
          ))}
        </select>
        <div className="omnibox">
          <div className="grow">
            <div className={cn('inputwrap', loading && 'inputwrap--loading')}>
              <Icon icon={Search} />
              {loading ? (
                <span className="loader" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                  <i />
                </span>
              ) : null}
              <input
                className="input"
                type="search"
                value={text}
                placeholder={t('busca.placeholder')}
                aria-label={t('busca.rotulo')}
                aria-describedby={`${id}-status`}
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => {
                  setText(event.target.value);
                }}
              />
            </div>
            <div className="omnibox__status" id={`${id}-status`} role="status">
              <StatusText
                loading={loading}
                query={query}
                total={Math.max(pages[0]?.total ?? 0, results.length)}
                sources={pages[0]?.sources ?? []}
              />
            </div>
          </div>
        </div>
        <Tooltip content={t('arquivo.indisponivel')}>
          <Button
            icon={FilePlus}
            aria-disabled="true"
            onClick={(event) => {
              event.preventDefault();
            }}
          >
            {t('arquivo.escolher')}
          </Button>
        </Tooltip>
      </div>

      {warnings.length > 0 ? (
        <div className="stack-2 preview__section">
          {warnings.map((warning) => (
            <WarningAlert
              key={`${warning.source}:${warning.reason}`}
              warning={warning}
              onRetry={() => {
                void search.refetch();
              }}
            />
          ))}
        </div>
      ) : null}

      <div className="disc">
        <FiltersColumn target={target} filters={filters} onChange={setFilters} sources={sources} />
        <div className="disc__main">
          {search.isPending ? (
            <ResultsSkeleton />
          ) : search.isError ? (
            <ErrorPanel
              title={t('erro')}
              error={search.error}
              onRetry={() => {
                void search.refetch();
              }}
            />
          ) : results.length === 0 ? (
            <EmptyResults
              query={query}
              target={target}
              includeIncompatible={filters.includeIncompatible}
              onShowIncompatible={() => {
                setFilters({ ...filters, includeIncompatible: true });
              }}
            />
          ) : (
            <>
              <ul className="dlist" aria-label={t('resultado.lista')}>
                {results.map((result) => (
                  <ResultRow
                    key={result.key}
                    result={result}
                    inPack={isInPack(result, inventoryKeys)}
                    selected={selected.has(result.key)}
                    current={current?.key === result.key}
                    onToggle={toggle}
                    onOpen={setCurrent}
                  />
                ))}
              </ul>
              <MoreResults
                hasMore={search.hasNextPage}
                loading={search.isFetchingNextPage}
                onMore={() => {
                  if (!search.isFetchingNextPage) void search.fetchNextPage();
                }}
              />
            </>
          )}
          {selected.size > 0 ? (
            <div className="selbar disc__selbar" role="region" aria-label={t('selecao.rotulo')}>
              <span className="selbar__count" aria-live="polite">
                {t('selecao.contagem', { count: selected.size })}
              </span>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setSelected(new Map());
                }}
              >
                {t('selecao.limpar')}
              </Button>
              <span className="grow" />
              <Button
                variant="primary"
                icon={Plus}
                onClick={() => {
                  openDialog([...selected.values()]);
                }}
              >
                {t('selecao.adicionar', { count: selected.size })}
              </Button>
            </div>
          ) : null}
        </div>
        {current ? (
          <Preview
            key={current.key}
            packId={packId}
            result={current}
            inPack={isInPack(current, inventoryKeys)}
            kind={kind}
            target={target}
            onAdd={(choice, title) => {
              setRequest({ choices: [choice], titles: [title] });
            }}
          />
        ) : (
          <aside className="panel disc__preview" aria-label={t('previa.rotuloVazio')}>
            <p className="t-sm t-3">{t('previa.nenhuma')}</p>
          </aside>
        )}
      </div>

      <DependenciesDialog
        packId={packId}
        request={request}
        target={target}
        onClose={() => {
          setRequest(null);
        }}
        onAdded={onAdded}
      />
    </div>
  );
}

function StatusText({
  loading,
  query,
  total,
  sources,
}: {
  loading: boolean;
  query: string;
  total: number;
  sources: readonly SearchResult['sources'][number]['source'][];
}) {
  const { t } = useTranslation('adicionar');
  if (loading) return <>{t('busca.buscando')}</>;
  if (query === '') return <>{t('busca.vazio')}</>;
  const fontes = t(`busca.fontes.${sourceLabel(sources.map((source) => ({ source })))}`);
  return <>{t('busca.resultados', { count: total, query, fontes })}</>;
}

function WarningAlert({ warning, onRetry }: { warning: SourceWarning; onRetry: () => void }) {
  const { t } = useTranslation('adicionar');
  const fonte = t(`fonte.${warning.source}`);
  if (warning.reason === 'unavailable') {
    return (
      <Alert
        kind="warn"
        title={t('avisos.indisponivel', { fonte })}
        actions={
          <Button size="sm" icon={RefreshCw} onClick={onRetry}>
            {t('acoes.tentarDeNovo', { ns: 'comum' })}
          </Button>
        }
      >
        <p>{t('avisos.indisponivelTexto')}</p>
      </Alert>
    );
  }
  const missing = warning.reason === 'keyMissing';
  return (
    <Alert
      kind="info"
      title={missing ? t('avisos.semChave') : t('avisos.chaveRecusada')}
      actions={
        <Link to="/configuracoes" className={buttonVariants({ size: 'sm' })}>
          {t('avisos.abrirConfiguracoes')}
        </Link>
      }
    >
      <p>{missing ? t('avisos.semChaveTexto') : t('avisos.chaveRecusadaTexto')}</p>
    </Alert>
  );
}

function ResultsSkeleton() {
  return (
    <ul className="dlist" aria-busy="true">
      {Array.from({ length: 5 }, (_, index) => (
        <li key={index} className="drow" aria-hidden="true">
          <Skeleton shape="tile" width="18px" />
          <Skeleton shape="tile" width="40px" />
          <span className="stack-2">
            <Skeleton width="45%" />
            <Skeleton width="80%" />
          </span>
          <span />
        </li>
      ))}
    </ul>
  );
}

function EmptyResults({
  query,
  target,
  includeIncompatible,
  onShowIncompatible,
}: {
  query: string;
  target: PackTarget;
  includeIncompatible: boolean;
  onShowIncompatible: () => void;
}) {
  const { t } = useTranslation('adicionar');
  const alvo = useTargetText(target);
  return (
    <EmptyState
      compact
      glyph="search"
      title={query ? t('vazio.titulo', { query }) : t('vazio.tituloSemTexto')}
      text={t('vazio.texto', { alvo })}
      actions={
        includeIncompatible ? undefined : (
          <Button variant="link" onClick={onShowIncompatible}>
            {t('vazio.mostrarSem')}
          </Button>
        )
      }
    />
  );
}

/** Rolagem infinita: a próxima página é pedida antes de chegar ao fim (e pelo botão). */
function MoreResults({
  hasMore,
  loading,
  onMore,
}: {
  hasMore: boolean;
  loading: boolean;
  onMore: () => void;
}) {
  const { t } = useTranslation('adicionar');
  const sentinel = useRef<HTMLDivElement>(null);
  const more = useRef(onMore);
  useEffect(() => {
    more.current = onMore;
  });
  useEffect(() => {
    const node = sentinel.current;
    if (!node || !hasMore || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) more.current();
      },
      { rootMargin: '600px 0px' },
    );
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  }, [hasMore]);
  if (!hasMore) return null;
  return (
    <div ref={sentinel} className="row disc__more">
      <Button size="sm" variant="ghost" loading={loading} onClick={onMore}>
        {loading ? t('resultado.carregandoMais') : t('resultado.carregarMais')}
      </Button>
      <span className="t-xs t-3">{t('resultado.rolarCarrega')}</span>
    </div>
  );
}
