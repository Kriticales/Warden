/**
 * Seção Configs do pack (SPEC T12; ADR-0067; esboço aprovado `design/bug-01/configs.html`,
 * opção 1 "Três colunas" com o "Comparar" da opção 3).
 *
 * - **Esquerda**: o título, o seletor **Do pack | Do teste local | Comparar**, uma busca só
 *   (nomes e conteúdo; os resultados ficam no lugar da árvore), os filtros "Mudou do padrão" e
 *   "Diferente do pack" em botões e a árvore em três grupos: No pack, Só no teste e Só neste
 *   computador (`places/`).
 * - **Meio**: o editor (Formulário | Texto) com a faixa do lugar; no teste, cada chave diferente
 *   do pack mostra "no pack: X" e **Trazer para o pack**, e no Texto as linhas diferentes ficam
 *   marcadas. No Comparar, o teste e o pack lado a lado com as setas → e ←.
 * - **Direita**: **Detalhes** (onde está, diferenças com o pack, avisos, Marcar como pessoal),
 *   recolhível pelo botão Detalhes.
 *
 * Por que juntar as origens: os mods criam as configs na instância de teste quando o jogo abre;
 * a seção que lia só o pack mostrava só a config do Crash Assistant.
 *
 * O arquivo, a vista e a linha moram na URL (`arquivo`, `origem`, `linha`). Com alterações não
 * salvas, qualquer saída (outro arquivo, outra vista, outra seção, fechar o app) pede
 * confirmação: Salvar e continuar, Descartar ou Continuar editando.
 */
import { useBlocker } from '@tanstack/react-router';
import { Columns2, Monitor, PanelRight, RotateCcw, Users, type LucideIcon } from 'lucide-react';
import { useCallback, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
} from '../../components/ui/alert-dialog';
import { Button } from '../../components/ui/button';
import type { ConfigOrigin, PackId } from '../../lib/ipc/bindings';
import { cn } from '../../lib/cn';
import { usePack } from '../packs/api';
import { useSettings } from '../settings/api';
import { useConfigFile } from './api';
import type { LineRequest } from './components/CodeEditor';
import { ConfigEditor, type EditorGuard } from './components/ConfigEditor';
import { ChangedFilter } from './form/defaults/ChangedFilter';
import { usePlaceActions } from './places/actions';
import { useConfigCompare, useConfigOverview } from './places/api';
import { CompareView } from './places/CompareView';
import { DetailsPanel } from './places/DetailsPanel';
import { EditorBand } from './places/EditorBand';
import { FilterChip } from './places/FilterChip';
import {
  byPath,
  differingCount,
  editorOrigin,
  filterFiles,
  groupFiles,
  paramOf,
  viewOf,
  type OriginParam,
  type View,
} from './places/model';
import { PlacesTree } from './places/PlacesTree';
import { useConfigSearch } from './search/api';
import { ConfigSearchField } from './search/ConfigSearchField';
import { ConfigSearchResults } from './search/ConfigSearchResults';
import { useScriptAddon } from './scripts/useScriptAddon';
import './configs.css';

export interface ConfigsSearch {
  /** A vista: `pack`, `instancia` (teste local) ou `comparar`. Sem ela, o teste local quando a
   * instância existe; senão, o pack. */
  origem?: OriginParam;
  /** Caminho do arquivo aberto. */
  arquivo?: string;
  /** Linha para ir no arquivo aberto (resultado da busca em todas as configs). */
  linha?: number;
}

export interface ConfigsPageProps {
  packId: PackId;
  search: ConfigsSearch;
  onNavigate: (search: ConfigsSearch) => void;
}

/** O painel Detalhes (o botão do editor o controla). */
const DETAILS_ID = 'cfgx-det';

/** Os detalhes abertos ou fechados valem para a seção toda nesta abertura do app. */
const remembered = { details: true };

const VIEWS: { view: View; icon: LucideIcon; label: 'pack' | 'test' | 'compare' }[] = [
  { view: 'pack', icon: Users, label: 'pack' },
  { view: 'test', icon: Monitor, label: 'test' },
  { view: 'compare', icon: Columns2, label: 'compare' },
];

export function ConfigsPage({ packId, search, onNavigate }: ConfigsPageProps) {
  const { t } = useTranslation('configs');
  const pack = usePack(packId);
  const settings = useSettings();
  const overview = useConfigOverview(packId);
  const testAvailable = overview.data?.testAvailable ?? false;
  const view = viewOf(search.origem, testAvailable, search.arquivo !== undefined);
  const path = search.arquivo ?? null;
  const placed = byPath(overview.data?.files, path);
  const origin: ConfigOrigin = editorOrigin(view, placed);
  const file = useConfigFile(packId, origin, path);
  const compareWanted = placed?.place === 'pack' && placed.inTest && placed.differs;
  const comparison = useConfigCompare(packId, path, compareWanted);
  const actions = usePlaceActions(packId);
  const [searchText, setSearchText] = useState('');
  const [resultsOpen, setResultsOpen] = useState(false);
  const [onlyChanged, setOnlyChanged] = useState(false);
  const [onlyDiffers, setOnlyDiffers] = useState(false);
  const [showDetails, setShowDetails] = useState(remembered.details);
  // Cada clique num resultado é um pedido novo, mesmo para a mesma linha.
  const [lineRequest, setLineRequest] = useState(0);
  const searching = resultsOpen && searchText.trim() !== '';
  const searchOrigin: ConfigOrigin = view === 'pack' || !testAvailable ? 'pack' : 'instance';
  const goTo: LineRequest | null =
    search.linha === undefined ? null : { line: search.linha, request: lineRequest };

  // "Mudou do padrão": os arquivos com alguma chave diferente do padrão (pelo índice da busca).
  const changed = useConfigSearch(packId, searchOrigin, '', null, true, true);
  const changedPaths = useMemo(
    () => new Set((changed.data?.results?.groups ?? []).map((group) => group.path)),
    [changed.data],
  );

  // As alterações não salvas do editor aberto; a referência é para a guarda ler o valor atual.
  const guard = useRef<EditorGuard | null>(null);
  const [dirty, setDirty] = useState(false);
  const onGuard = useCallback((next: EditorGuard | null) => {
    guard.current = next;
    setDirty(next?.dirty ?? false);
  }, []);

  // Sem `origem` na URL vale a vista calculada: escolher outra linha do mesmo arquivo não é
  // trocar de vista.
  const originOf = (value: unknown) => (value as ConfigsSearch).origem ?? paramOf(view);
  const blocker = useBlocker({
    // Ir para outra linha do mesmo arquivo não sai dele.
    shouldBlockFn: ({ current, next }) =>
      (guard.current?.dirty ?? false) &&
      !(
        current.pathname === next.pathname &&
        (current.search as ConfigsSearch).arquivo === (next.search as ConfigsSearch).arquivo &&
        originOf(current.search) === originOf(next.search)
      ),
    enableBeforeUnload: () => guard.current?.dirty ?? false,
    withResolver: true,
  });

  const files = overview.data?.files ?? [];
  const filtered = filterFiles(files, {
    changed: onlyChanged ? changedPaths : null,
    differs: onlyDiffers && view !== 'pack',
  });
  const groups = groupFiles(filtered, view);
  const onlyTestCount = files.filter((entry) => entry.place === 'test').length;
  const changedCount = files.filter((entry) => changedPaths.has(entry.path)).length;
  const filtering = onlyChanged || (onlyDiffers && view !== 'pack');

  const go = (next: View, nextPath: string | null = path, line?: number) => {
    onNavigate({
      origem: paramOf(next),
      ...(nextPath === null ? {} : { arquivo: nextPath }),
      ...(line === undefined ? {} : { linha: line }),
    });
  };
  const select = (nextPath: string, line?: number) => {
    go(view, nextPath, line);
  };
  const clearSearch = () => {
    setSearchText('');
    setResultsOpen(false);
  };
  const toggleDetails = () => {
    remembered.details = !showDetails;
    setShowDetails(!showDetails);
  };

  // Scripts KubeJS e CraftTweaker abrem com os recursos do editor de scripts (C-07; T26).
  const onOpenFile = (next: string) => {
    onNavigate({ origem: paramOf(view), arquivo: next });
  };
  const onOpenInInstance = (next: string) => {
    onNavigate({ origem: 'instancia', arquivo: next });
  };
  const scriptAddon = useScriptAddon({
    packId,
    origin,
    path,
    loader: pack.data?.loader ?? null,
    minecraft: pack.data?.minecraft ?? null,
    onOpenFile,
    onOpenInInstance,
  });

  const onViewKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const index = VIEWS.findIndex((entry) => entry.view === view);
    const step = event.key === 'ArrowRight' ? 1 : -1;
    const next = VIEWS[(index + step + VIEWS.length) % VIEWS.length];
    if (!next) return;
    go(next.view);
    const group = event.currentTarget.parentElement;
    requestAnimationFrame(() => {
      group?.querySelector<HTMLButtonElement>(`[data-view="${next.view}"]`)?.focus();
    });
  };

  const perKeys = (comparison.data?.keys ?? []).filter((key) => key.perKey).map((key) => key.key);
  const packDiff =
    placed?.place === 'pack' && placed.differs && comparison.data?.byKey
      ? {
          keys: new Map(comparison.data.keys.map((key) => [key.key, key])),
          side: origin === 'instance' ? ('test' as const) : ('pack' as const),
          packText: comparison.data.packText,
          busyKey: actions.busy?.startsWith(`${placed.path}|`)
            ? actions.busy.slice(placed.path.length + 1)
            : null,
          ...(origin === 'instance'
            ? {
                onBring: (key: string) => {
                  actions.bring(placed.path, [key]);
                },
              }
            : {}),
        }
      : placed?.place === 'pack' && placed.differs && comparison.data && origin === 'instance'
        ? {
            keys: new Map(),
            side: 'test' as const,
            packText: comparison.data.packText,
          }
        : null;

  const detailsButton = placed ? (
    <Button
      size="sm"
      icon={PanelRight}
      aria-expanded={showDetails}
      aria-controls={DETAILS_ID}
      className={cn('cfgx__det-toggle', showDetails && 'is-on')}
      onClick={toggleDetails}
    >
      {t('detalhes.botao')}
    </Button>
  ) : null;

  const compareMode = view === 'compare' && placed?.place === 'pack' && placed.inTest;
  // `compareMode` já garante o arquivo; o TypeScript não acompanha.
  const compared = compareMode ? placed : undefined;

  let main;
  if (overview.isPending) {
    main = <LoadingState label={t('carregando')} />;
  } else if (overview.isError) {
    main = (
      <ErrorPanel
        title={t('erroArvore')}
        error={overview.error}
        onRetry={() => {
          void overview.refetch();
        }}
      />
    );
  } else if (view !== 'pack' && !testAvailable) {
    main = (
      <EmptyState
        glyph="dots"
        title={t('vazio.instanciaTitulo')}
        text={t('vazio.instanciaTexto')}
      />
    );
  } else if (files.length === 0) {
    main = <EmptyState glyph="dots" title={t('vazio.titulo')} text={t('vazio.texto')} />;
  } else if (path === null) {
    main = (
      <EmptyState
        compact
        glyph="box"
        title={t('vazio.semArquivoTitulo')}
        text={t('vazio.semArquivoTexto')}
      />
    );
  } else if (compared) {
    main = comparison.isSuccess ? (
      <div className="cfgx__compare">
        <div className="editor__bar cfg-edhead">
          <span className="cfg-edhead__t">
            <span className="cfg-edhead__name">{compared.path.split('/').pop()}</span>
            <span className="editor__file cfg-edhead__path">{compared.path}</span>
          </span>
          {detailsButton}
        </div>
        <CompareView file={compared} comparison={comparison.data} actions={actions} />
      </div>
    ) : comparison.isError ? (
      <ErrorPanel title={t('editor.erroArquivo', { arquivo: path })} error={comparison.error} />
    ) : compared.differs ? (
      <LoadingState label={t('editor.carregandoArquivo')} />
    ) : (
      <div className="cfgx__compare">
        <div className="editor__bar cfg-edhead">
          <span className="cfg-edhead__t">
            <span className="cfg-edhead__name">{compared.path.split('/').pop()}</span>
            <span className="editor__file cfg-edhead__path">{compared.path}</span>
          </span>
          {detailsButton}
        </div>
        <div className="cmp">
          <div className="cmp__sum">
            <span className="cmp__sum-t">
              <b>{t('comparar.igual')}</b> {t('comparar.igualTexto')}
            </span>
          </div>
        </div>
      </div>
    );
  } else if (file.isPending) {
    main = <LoadingState label={t('editor.carregandoArquivo')} />;
  } else if (file.isError) {
    main = (
      <ErrorPanel
        title={t('editor.erroArquivo', { arquivo: path })}
        error={file.error}
        onRetry={() => {
          void file.refetch();
        }}
      />
    );
  } else {
    main = (
      <ConfigEditor
        key={`${origin}:${file.data.path}`}
        packId={packId}
        origin={origin}
        file={file.data}
        loader={pack.data?.loader ?? null}
        minecraft={pack.data?.minecraft ?? null}
        showDiff={settings.data?.configDiffBeforeSave ?? true}
        onGuard={onGuard}
        goTo={goTo}
        addon={scriptAddon}
        headActions={detailsButton}
        packDiff={packDiff}
        band={
          <>
            {view === 'compare' && placed && placed.place !== 'pack' ? (
              <div className="band band--own" role="note">
                <span className="band__t">{t('comparar.soVale')}</span>
              </div>
            ) : null}
            {placed ? (
              <EditorBand
                file={placed}
                view={view}
                testAvailable={testAvailable}
                perKeys={perKeys}
                actions={actions}
                onShowTest={() => {
                  go('test');
                }}
              />
            ) : null}
          </>
        }
      />
    );
  }

  return (
    <div className={cn('cfgx', showDetails && placed ? 'cfgx--det' : '')}>
      <aside className="cfgx__nav" aria-labelledby="cfgx-title">
        <div className="cfgx__colhead">
          <h1 id="cfgx-title">{t('titulo')}</h1>
          <Button
            size="sm"
            variant="ghost"
            iconOnly
            icon={RotateCcw}
            onClick={() => {
              void overview.refetch();
              void file.refetch();
            }}
          >
            {t('recarregar')}
          </Button>
        </div>
        <div className="cfgx__top">
          <div className="cfgx__origin">
            <div
              className="segmented segmented--wide"
              role="radiogroup"
              aria-label={t('origem.rotulo')}
            >
              {VIEWS.map((entry) => {
                const disabled = entry.view !== 'pack' && !testAvailable && overview.isSuccess;
                return (
                  <button
                    key={entry.view}
                    type="button"
                    role="radio"
                    data-view={entry.view}
                    aria-checked={view === entry.view}
                    aria-disabled={disabled ? true : undefined}
                    title={disabled ? t('origem.semTeste') : undefined}
                    tabIndex={view === entry.view ? 0 : -1}
                    onKeyDown={onViewKey}
                    onClick={() => {
                      if (!disabled) go(entry.view);
                    }}
                  >
                    <entry.icon className="icon" aria-hidden="true" />
                    <span>{t(`origem.${entry.label}`)}</span>
                  </button>
                );
              })}
            </div>
            <p className={cn('cfgx__cap', `cfgx__cap--${view}`)}>
              {view === 'pack' ? (
                <>
                  <b>{t('origem.legendaPack')}</b> {t('origem.legendaPackTexto')}
                </>
              ) : view === 'test' ? (
                <>
                  <b>{t('origem.legendaTeste')}</b> {t('origem.legendaTesteTexto')}
                </>
              ) : (
                <>
                  <b>{t('origem.legendaComparar')}</b> {t('origem.legendaCompararTexto')}
                </>
              )}
            </p>
          </div>
          <ConfigSearchField
            packId={packId}
            origin={searchOrigin}
            value={searchText}
            onChange={(value) => {
              setSearchText(value);
              setResultsOpen(true);
            }}
            onFocus={() => {
              setResultsOpen(true);
            }}
            onClear={clearSearch}
          />
          <div className="fchips fchips--stack" role="group" aria-label={t('filtros.rotulo')}>
            <ChangedFilter
              long
              checked={onlyChanged}
              onChange={setOnlyChanged}
              count={changedCount}
            />
            {view === 'pack' ? null : (
              <FilterChip
                label={t('filtros.difere')}
                place="test"
                pressed={onlyDiffers}
                count={differingCount(files)}
                title={t('filtros.difereDica')}
                onChange={setOnlyDiffers}
              />
            )}
          </div>
        </div>
        <div className="cfgx__list">
          {searching ? (
            <ConfigSearchResults
              packId={packId}
              origin={searchOrigin}
              query={searchText}
              onOpen={(nextPath, line) => {
                setResultsOpen(false);
                setLineRequest((count) => count + 1);
                select(nextPath, line);
              }}
              onClear={clearSearch}
            />
          ) : overview.isSuccess ? (
            <>
              {overview.data.truncated ? (
                <p className="ctree__note">{t('arvore.cortada', { max: '20.000' })}</p>
              ) : null}
              {filtering && filtered.length === 0 ? (
                <p className="ctree__note">{t('filtros.nenhum')}</p>
              ) : (
                <PlacesTree
                  key={view}
                  groups={groups}
                  view={view}
                  selected={path}
                  dirty={dirty ? path : null}
                  expandAll={filtering}
                  onlyTestCount={onlyTestCount}
                  onSelect={select}
                  onShowTest={() => {
                    go('test', null);
                  }}
                />
              )}
            </>
          ) : null}
        </div>
        {view === 'pack' ? <p className="cfgx__foot">{t('grupos.rodapePack')}</p> : null}
      </aside>

      <div className="cfgx__main">{main}</div>

      {showDetails && placed ? (
        <div id={DETAILS_ID} className="cfgx__det-wrap">
          <DetailsPanel file={placed} view={view} comparison={comparison.data} actions={actions} />
        </div>
      ) : null}

      {actions.dialogs}

      <AlertDialog
        open={blocker.status === 'blocked'}
        onOpenChange={(open) => {
          if (!open) blocker.reset?.();
        }}
      >
        <AlertDialogContent
          title={t('sair.titulo')}
          description={t('sair.texto', { arquivo: path ?? '' })}
          footer={
            <>
              <AlertDialogCancel asChild>
                <Button variant="ghost">{t('sair.ficar')}</Button>
              </AlertDialogCancel>
              <Button
                variant="danger"
                onClick={() => {
                  blocker.proceed?.();
                }}
              >
                {t('sair.descartar')}
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  void (guard.current?.save() ?? Promise.resolve(true)).then((saved) => {
                    if (saved) blocker.proceed?.();
                    else blocker.reset?.();
                  });
                }}
              >
                {t('sair.salvar')}
              </Button>
            </>
          }
        />
      </AlertDialog>
    </div>
  );
}
