/**
 * Página Adicionar ao pack (SPEC T08; ESTRUTURA N6; esboço aprovado
 * `design/bug-01/adicionar.html`, opção 4 · As duas, com seletor; ADR-0068): em tela cheia, com
 * o menu do pack recolhido (a rota declara `packMenu: 'compact'`) e o cabeçalho do pack visível.
 *
 * Duas colunas: à esquerda o tipo (Mods, Resource packs, Shaders, Modpacks), o campo único de
 * busca, o seletor **Lista | Cartões** (o app lembra o último modo; o que foi marcado continua
 * marcado ao trocar), a linha discreta com a trava de versão, os resultados e a barra dos
 * marcados; à direita os filtros, que dão lugar ao detalhe do mod quando você clica no nome
 * dele. Adicionar abre sempre **um** diálogo Adicionar itens (T09) para o conjunto; depois de
 * gravar, a busca continua aberta e os itens passam a mostrar "No pack".
 *
 * Com o campo vazio e sem filtros, o início (P1-16): "Populares" e "Atualizados recentemente",
 * mais os kits. Digitar, escolher uma categoria ou mexer num filtro de busca troca o início pela
 * busca. Um link colado no campo é lido no lugar da busca: da CurseForge (P1-10), pelo packwiz
 * numa cópia do pack (ARCHITECTURE §6.1); do Modrinth, pela API; direto `https://`, baixado uma
 * vez; `http://`, recusado sem baixar nada (P1-11). Arquivos escolhidos no computador ou
 * arrastados para a janela abrem a lista de arquivos (`local/`), com a identificação pelo hash e
 * o bloqueio de loader ou versão.
 *
 * Com Tipo = Modpacks (P1-17), os resultados são modpacks sem caixa de seleção (um modpack nunca
 * entra no pack) e abrir um deles mostra, no lugar dos filtros, a lista de mods dele
 * (`modpacks/`), cuja seleção abre o mesmo diálogo.
 */
import { Link } from '@tanstack/react-router';
import { ArrowLeft, FilePlus, LayoutGrid, List, Lock, Search, X } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { PageTitle } from '../../app/layout/PageHead';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Skeleton } from '../../components/common/LoadingState';
import { Button, buttonVariants } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Segmented } from '../../components/ui/segmented';
import { cn } from '../../lib/cn';
import type {
  AddResult,
  HashMatch,
  LinkTarget,
  LocalFilesDropped,
  ModrinthLinkTarget,
  PackId,
  ProjectKind,
  SearchResult,
  SourceId,
} from '../../lib/ipc/bindings';
import { loaderName } from '../packs/lib/pack-list';
import { useInventory } from '../pack-editor/api';
import { usePack } from '../packs/api';
import { useSearch } from './common/api';
import {
  activeFilterCount,
  FiltersColumn,
  NO_FILTERS,
  searchFilters,
  type UserFilters,
} from './common/FiltersColumn';
import { isInPack, knownSources, mergePages, pageWarnings, sourceLabel } from './common/model';
import { type PackTarget, SourceLegend, useTargetText } from './common/text';
import { WarningAlert } from './common/WarningAlert';
import type { LinkPick } from './curseforge/api';
import { resultOfLink } from './curseforge/link';
import { CurseforgeLinkPanel } from './curseforge/LinkPanel';
import { DependenciesDialog, type DependenciesRequest } from './dependencies/DependenciesDialog';
import { useDiscoverCategories } from './discover/api';
import { DiscoverHome } from './discover/DiscoverHome';
import { DirectLinkPanel } from './link/DirectLinkPanel';
import { classifyLink } from './link/link';
import { ModrinthLinkPanel, resultOfModrinthLink } from './link/ModrinthLinkPanel';
import { RefusedLink } from './link/RefusedLink';
import { useChooseFiles } from './local/api';
import { takePendingDrop, useDroppedFiles } from './local/dropped';
import { LocalFilesPanel } from './local/LocalFilesPanel';
import { ModpackPanel } from './modpacks/ModpackPanel';
import { ModpackRow } from './modpacks/ModpackRow';
import { Preview } from './preview/Preview';
import { readViewMode, saveViewMode, type ViewMode } from './results/model';
import { ResultsView, type ResultsViewProps } from './results/ResultsView';
import { choicesOf, SelectionBar } from './results/SelectionBar';
import './common/add.css';

export const PROJECT_KINDS: readonly ProjectKind[] = ['mod', 'resourcePack', 'shader', 'modpack'];

/** Espera entre a última tecla e a busca. */
const TYPING_DELAY_MS = 300;

/** A categoria curada das bibliotecas: escolhida, elas aparecem. */
const LIBRARIES_CATEGORY = 'bibliotecas';

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
  // Um link não é um nome para buscar: cada tipo de link tem o seu painel.
  const linkKind = classifyLink(text);
  const typedLink = linkKind === null ? null : text.trim();
  // O link também espera a última tecla: digitado letra a letra, cada pedaço seria lido.
  const debouncedLink = useDebounced(typedLink, TYPING_DELAY_MS);
  const linkUrl = typedLink !== null && debouncedLink === typedLink ? typedLink : null;
  const query = useDebounced(typedLink === null ? text.trim() : '', TYPING_DELAY_MS);
  // A última tecla ainda não virou busca: nada é pedido às fontes com o texto anterior (ao
  // sair do início, seria uma busca vazia que troca a lista logo depois, levando o foco junto).
  const typing = typedLink === null && text.trim() !== query;
  const [filters, setFilters] = useState<UserFilters>(NO_FILTERS);
  const [view, setView] = useState<ViewMode>(() => readViewMode());
  // O início aparece com o campo vazio e sem nenhum filtro de busca; qualquer um deles liga a
  // busca. Modpacks não têm início: o campo vazio já lista os modpacks para o pack.
  const modpacks = kind === 'modpack';
  const showHome =
    !modpacks && query === '' && text.trim() === '' && activeFilterCount(filters) === 0;
  const search = useSearch(
    packId,
    { query, kind, ...searchFilters(filters) },
    !showHome && !typing,
  );
  const [selected, setSelected] = useState<ReadonlyMap<string, SearchResult>>(new Map());
  const [current, setCurrent] = useState<SearchResult | null>(null);
  const [request, setRequest] = useState<DependenciesRequest | null>(null);
  const [added, setAdded] = useState(0);
  // O projeto do link aberto no detalhe, e a versão escolhida lá para o packwiz ler.
  const [linkProject, setLinkProject] = useState<{ url: string; projectId: string } | null>(null);
  const [linkPick, setLinkPick] = useState<LinkPick | null>(null);
  // Arquivos do computador (escolhidos ou arrastados), aberto no lugar dos resultados.
  const [local, setLocal] = useState<LocalFilesDropped | null>(() => takePendingDrop());
  const chooseFiles = useChooseFiles(packId);
  const categories = useDiscoverCategories(kind);
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

  const changeView = (next: ViewMode) => {
    setView(next);
    saveViewMode(next);
  };

  const openDialog = (items: readonly SearchResult[]) => {
    setRequest({ choices: choicesOf(items), titles: items.map((result) => result.title) });
  };

  const onAdded = (result: AddResult) => {
    setAdded((count) => count + result.added.length);
    setSelected(new Map());
    setRequest(null);
  };

  const onLinkProject = (found: LinkTarget) => {
    if (found.kind !== kind) onKindChange(found.kind);
    setSelected(new Map());
    setLinkPick(null);
    if (linkUrl !== null) setLinkProject({ url: linkUrl, projectId: found.projectId });
    setCurrent(resultOfLink(found));
  };

  const onLinkFile = (found: LinkTarget) => {
    setLinkPick(null);
    setRequest({
      choices: [{ source: 'curseforge', projectId: found.projectId, versionId: found.fileId }],
      titles: [found.title],
    });
  };

  const onModrinthProject = (found: ModrinthLinkTarget) => {
    if (found.kind !== kind) onKindChange(found.kind);
    setSelected(new Map());
    setCurrent(resultOfModrinthLink(found));
  };

  const onModrinthVersion = (found: ModrinthLinkTarget) => {
    setRequest({
      choices: [{ source: 'modrinth', projectId: found.projectId, versionId: found.versionId }],
      titles: [found.title],
    });
  };

  // Arquivo achado pelo hash: a referência entra pelo caminho normal, sem copiar o jar.
  const onReference = (match: HashMatch) => {
    setRequest({
      choices: [{ source: match.source, projectId: match.projectId, versionId: match.versionId }],
      titles: [match.title],
    });
  };

  const onFilesAdded = (result: AddResult) => {
    setAdded((count) => count + result.added.length);
  };

  // A lista de arquivos ocupa o lugar dos resultados: o detalhe aberto fecha.
  const openLocal = (files: LocalFilesDropped) => {
    setCurrent(null);
    setLocal(files);
  };

  useDroppedFiles((dropped) => {
    if (dropped.files.length > 0 || dropped.ignored > 0) openLocal(dropped);
  });

  const changeKind = (next: ProjectKind) => {
    setSelected(new Map());
    setCurrent(null);
    // As categorias são de cada tipo: trocar o tipo volta os filtros ao começo.
    setFilters(NO_FILTERS);
    onKindChange(next);
  };

  const modpackOpen = modpacks && current !== null;

  const loading =
    typedLink === null &&
    !showHome &&
    (search.isPending || (search.isFetching && !search.isFetchingNextPage));

  // A Lista e os Cartões mostram as mesmas coisas, do mesmo jeito.
  const resultsProps: Omit<ResultsViewProps, 'groups'> = {
    view,
    options: {
      hideLibraries:
        kind === 'mod' &&
        !filters.showLibraries &&
        query === '' &&
        filters.category !== LIBRARIES_CATEGORY,
      showInPack: filters.showInPack,
      inventoryKeys,
    },
    selected,
    currentKey: current?.key ?? null,
    onToggle: toggle,
    onOpen: setCurrent,
    onShowLibraries: () => {
      setFilters((now) => ({ ...now, showLibraries: true }));
    },
    sort: filters.sort,
    onSort: (sort) => {
      setFilters((now) => ({ ...now, sort }));
    },
  };
  const categoryName =
    filters.category === null
      ? null
      : (categories.data?.find((category) => category.id === filters.category)?.name ?? null);
  const groupTitle =
    query !== ''
      ? t('resultado.resultadosPara', { query })
      : (categoryName ?? t('resultado.lista'));
  // As fontes da linha da trava: as da busca (ou do início) ou, antes delas, as duas.
  const lockSources: readonly SourceId[] =
    sources.length > 0 ? sources : ['modrinth', 'curseforge'];

  return (
    <div
      className={cn(
        'add-page',
        current && !modpacks && 'add-page--detail',
        modpackOpen && 'add-page--modpack',
      )}
    >
      <div className="add-main">
        <div className="addhead">
          <Link
            to="/packs/$packId/mods"
            params={{ packId }}
            className={cn(buttonVariants({ variant: 'ghost', size: 'sm' }), 'addhead__back')}
            title={t('voltarDica')}
            aria-label={
              added > 0 ? t('voltarRotuloAdicionados', { count: added }) : t('voltarRotulo')
            }
          >
            <Icon icon={ArrowLeft} size="sm" />
            <span>{t('voltar')}</span>
            {added > 0 ? (
              <span className="addhead__added">{t('adicionados', { count: added })}</span>
            ) : null}
          </Link>
          <span className="addhead__sep" aria-hidden="true" />
          <PageTitle className="addhead__title">{t('titulo')}</PageTitle>
          <span className="grow" />
          <Button
            icon={FilePlus}
            title={t('arquivo.dica')}
            aria-label={t('arquivo.escolher')}
            loading={chooseFiles.isPending}
            onClick={() => {
              chooseFiles.mutate(undefined, {
                onSuccess: (files) => {
                  if (files.length > 0) openLocal({ files, ignored: 0 });
                },
              });
            }}
          >
            <span className="wide-only">{t('arquivo.escolher')}</span>
            <span className="narrow-only">{t('arquivo.escolherCurto')}</span>
          </Button>
        </div>

        <div className="addsearch" role="search">
          <div className="addsearch__row">
            <Segmented
              className="addsearch__kind"
              label={t('tipo.rotulo')}
              value={kind}
              options={PROJECT_KINDS.map((value) => ({ value, label: t(`tipo.${value}`) }))}
              onChange={changeKind}
            />
            <select
              className="select addsearch__kindselect"
              aria-label={t('tipo.rotulo')}
              value={kind}
              onChange={(event) => {
                const next = PROJECT_KINDS.find((value) => value === event.target.value);
                if (next) changeKind(next);
              }}
            >
              {PROJECT_KINDS.map((value) => (
                <option key={value} value={value}>
                  {t(`tipo.${value}`)}
                </option>
              ))}
            </select>
            <div className={cn('inputwrap addsearch__field', loading && 'inputwrap--loading')}>
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
            {modpacks ? null : (
              <Segmented
                className="addsearch__view"
                label={t('modo.rotulo')}
                value={view}
                options={[
                  { value: 'lista', label: t('modo.lista'), icon: List, hint: t('modo.listaDica') },
                  {
                    value: 'cartoes',
                    label: t('modo.cartoes'),
                    icon: LayoutGrid,
                    hint: t('modo.cartoesDica'),
                  },
                ]}
                onChange={changeView}
              />
            )}
          </div>
          <LockLine target={target} sources={lockSources} />
          <div className="sr-only" id={`${id}-status`} role="status">
            <StatusText
              loading={loading}
              query={query}
              total={Math.max(pages[0]?.total ?? 0, results.length)}
              sources={pages[0]?.sources ?? []}
            />
          </div>
        </div>

        <div className="addscroll">
          {!showHome && warnings.length > 0 ? (
            <div className="stack-2 addscroll__warn">
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
          {chooseFiles.isError ? (
            <ErrorPanel compact title={t('erro')} error={chooseFiles.error} />
          ) : null}
          {local !== null ? (
            <LocalFilesPanel
              key={local.files.map((file) => file.token).join(',')}
              packId={packId}
              files={local.files}
              ignored={local.ignored}
              target={target}
              onReference={onReference}
              onAdded={onFilesAdded}
              onBack={() => {
                setLocal(null);
              }}
            />
          ) : linkKind === 'insecure' || linkKind === 'unsupported' ? (
            <RefusedLink kind={linkKind} />
          ) : typedLink !== null && linkUrl === null ? (
            <ResultsSkeleton />
          ) : linkUrl !== null && linkKind === 'modrinth' ? (
            <ModrinthLinkPanel
              key={linkUrl}
              packId={packId}
              url={linkUrl}
              onProject={onModrinthProject}
              onVersion={onModrinthVersion}
            />
          ) : linkUrl !== null && linkKind === 'direct' ? (
            <DirectLinkPanel
              key={linkUrl}
              packId={packId}
              url={linkUrl}
              target={target}
              onReference={onReference}
              onAdded={onFilesAdded}
              onBack={() => {
                setText('');
              }}
            />
          ) : linkUrl !== null ? (
            <CurseforgeLinkPanel
              key={linkUrl}
              packId={packId}
              url={linkUrl}
              onProject={onLinkProject}
              onFile={onLinkFile}
              pick={linkProject?.url === linkUrl ? linkPick : null}
            />
          ) : showHome ? (
            <DiscoverHome
              packId={packId}
              kind={kind}
              target={target}
              results={resultsProps}
              onSeeMore={(sort) => {
                setFilters({ ...filters, sort });
              }}
              onAdded={(result) => {
                setAdded((count) => count + result.added.length);
              }}
            />
          ) : search.isPending ? (
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
              {modpacks ? (
                <ul className="dlist" aria-label={t('resultado.lista')}>
                  {results.map((result) => (
                    <ModpackRow
                      key={result.key}
                      result={result}
                      current={current?.key === result.key}
                      onOpen={setCurrent}
                    />
                  ))}
                </ul>
              ) : (
                <ResultsView
                  {...resultsProps}
                  groups={[{ key: 'resultados', title: groupTitle, items: results, total: null }]}
                />
              )}
              <MoreResults
                hasMore={search.hasNextPage}
                loading={search.isFetchingNextPage}
                onMore={() => {
                  if (!search.isFetchingNextPage) void search.fetchNextPage();
                }}
              />
            </>
          )}
        </div>
        {selected.size > 0 ? (
          <SelectionBar
            packId={packId}
            selected={[...selected.values()]}
            onClear={() => {
              setSelected(new Map());
            }}
            onAdd={() => {
              openDialog([...selected.values()]);
            }}
          />
        ) : null}
      </div>

      <div className="add-side">
        {modpackOpen ? (
          <ModpackPanel
            key={current.key}
            packId={packId}
            result={current}
            target={target}
            inventoryKeys={inventoryKeys}
            onAdd={(choices, titles) => {
              setRequest({ choices, titles });
            }}
            onClose={() => {
              setCurrent(null);
            }}
          />
        ) : current && !modpacks ? (
          <div className="add-side__detail" role="region" aria-label={t('filtros.detalheRotulo')}>
            <div className="sidehead sidehead--back">
              <Button
                size="sm"
                variant="ghost"
                icon={ArrowLeft}
                onClick={() => {
                  setCurrent(null);
                }}
              >
                {t('filtros.detalheVoltar')}
              </Button>
              <span className="grow" />
              <Button
                size="sm"
                variant="ghost"
                iconOnly
                icon={X}
                onClick={() => {
                  setCurrent(null);
                }}
              >
                {t('filtros.detalheFechar')}
              </Button>
            </div>
            <Preview
              key={current.key}
              packId={packId}
              result={current}
              inPack={isInPack(current, inventoryKeys)}
              kind={kind}
              target={target}
              onAdd={(choice, title) => {
                // Do link de projeto: quem lê a versão escolhida é o packwiz (CA-T08-03).
                const fileId = choice.versionId;
                const fromLink =
                  linkUrl !== null &&
                  linkProject?.url === linkUrl &&
                  choice.source === 'curseforge' &&
                  choice.projectId === linkProject.projectId;
                if (fromLink && typeof fileId === 'string') {
                  setLinkPick({ projectId: choice.projectId, fileId });
                } else {
                  setRequest({ choices: [choice], titles: [title] });
                }
              }}
            />
          </div>
        ) : (
          <FiltersColumn kind={kind} filters={filters} onChange={setFilters} sources={sources} />
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

/** "🔒 Só versões para Minecraft 1.21.1 com NeoForge, no Modrinth e na CurseForge." */
function LockLine({ target, sources }: { target: PackTarget; sources: readonly SourceId[] }) {
  const { t } = useTranslation('adicionar');
  const fontes = t(`trava.fontes.${sourceLabel(sources.map((source) => ({ source })))}`);
  return (
    <p className="lockline">
      <Icon icon={Lock} size="sm" />
      <span>
        {target.loader ? (
          <Trans
            t={t}
            i18nKey="trava.texto"
            values={{ minecraft: target.minecraft, loader: loaderName(target.loader), fontes }}
            components={{ b: <b /> }}
          />
        ) : (
          <Trans
            t={t}
            i18nKey="trava.textoSemLoader"
            values={{ minecraft: target.minecraft, fontes }}
            components={{ b: <b /> }}
          />
        )}
      </span>
      <SourceLegend sources={sources} />
    </p>
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

function ResultsSkeleton() {
  return (
    <div className="tbl" aria-busy="true">
      {Array.from({ length: 8 }, (_, index) => (
        <div key={index} className="trow" aria-hidden="true">
          <Skeleton shape="tile" width="18px" />
          <Skeleton width="70%" />
          <Skeleton width="85%" />
          <Skeleton width="80%" />
          <Skeleton width="80%" />
          <span />
          <span />
        </div>
      ))}
    </div>
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
