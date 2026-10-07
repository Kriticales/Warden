/**
 * Console do jogo (SPEC T13, "Console"): linhas coloridas por nível, com hora e origem; filtro
 * de nível, busca, pausar a rolagem, copiar seleção, copiar tudo e salvar em arquivo. Mostra
 * até 50.000 linhas numa lista virtualizada; o log completo fica gravado na sessão.
 *
 * Rolagem: com o jogo aberto, acompanha o fim a cada lote até o usuário pausar (botão, rolar
 * para cima com a roda, as setas, Page Up ou Home, ou arrastar a barra). Pausado, mostra
 * quantas linhas chegaram e "Ir para o fim", que retoma. Com o jogo fechado, abre no fim e
 * não se mexe mais sozinho.
 */
import { measureElement, useVirtualizer } from '@tanstack/react-virtual';
import { ChevronDown, Copy, Download, Pause, Play, Search, Square, TextSelect } from 'lucide-react';
import {
  useDeferredValue,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type JSX,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
  type WheelEvent,
} from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import { showToast } from '../../../components/ui/toast';
import { cn } from '../../../lib/cn';
import { formatInteger } from '../../../lib/format';
import type { ConsoleLine } from '../../../lib/ipc/bindings';
import { log } from '../../../lib/log';
import { consoleLineAsText, consoleText, consoleTime } from '../text';
import {
  filterLines,
  highlightRanges,
  lineTone,
  normalizeForSearch,
  type ConsoleRow,
  type LevelFilter,
} from './filter';

import './console.css';

export type ConsoleState = 'live' | 'ended' | 'waiting';

export interface ConsoleProps {
  /** Até 50.000 linhas; a cada lote chega um array novo, com as antigas e as novas. */
  lines: readonly ConsoleLine[];
  /** Jogo aberto, jogo fechado ou esperando o jogo abrir. */
  state: ConsoleState;
  /** Nome acessível; padrão "Console do jogo". */
  label?: string;
  /** "Salvar em arquivo…"; sem ele, o botão não aparece. */
  onSave?: (() => void) | undefined;
  className?: string;
}

/** Altura estimada de uma linha sem quebra (12 px × 1,65). */
const ROW_ESTIMATE = 20;
/** Linhas desenhadas além das visíveis, para a rolagem não piscar. */
const OVERSCAN = 20;
/** Respiro de cima e de baixo do log (`--space-2`), dentro da lista virtualizada. */
const LOG_PADDING = 8;
/** Distância do fim, em px, que ainda conta como "no fim" ao arrastar a barra. */
const END_TOLERANCE = 4;
/** O seletor de nível: valor e texto do catálogo. */
const LEVEL_OPTIONS = [
  ['all', 'console.nivel.todas'],
  ['warn', 'console.nivel.avisos'],
  ['error', 'console.nivel.erros'],
] as const satisfies readonly (readonly [LevelFilter, string])[];
/** Teclas que rolam para cima no log (e por isso pausam a rolagem). */
const UP_KEYS = new Set(['ArrowUp', 'PageUp', 'Home']);

/** Texto em que a busca procura (mostrado + origem), normalizado uma vez por linha. */
const searchCache = new WeakMap<ConsoleLine, string>();
function searchable(line: ConsoleLine): string {
  let text = searchCache.get(line);
  if (text === undefined) {
    text = normalizeForSearch(`${consoleText(line)}\n${line.logger ?? ''}`);
    searchCache.set(line, text);
  }
  return text;
}

/** Quantas linhas chegaram depois da linha `seq` (as linhas vêm em ordem de `seq`). */
function countAfter(lines: readonly ConsoleLine[], seq: number): number {
  let count = 0;
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    const line = lines[i];
    if (!line || line.seq <= seq) break;
    count += 1;
  }
  return count;
}

/**
 * O texto da seleção dentro do log. Dentro de uma mensagem só, o trecho selecionado; pegando
 * mais de uma coluna ou linha, as linhas inteiras ("14:20:05 [INFO] [origem] mensagem"), uma
 * por linha, porque as colunas coladas pelo navegador ficariam ilegíveis.
 */
function selectedText(logElement: HTMLElement, rows: readonly ConsoleRow[]): string {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
    return '';
  }
  const raw = selection.toString();
  const range = selection.getRangeAt(0);
  const container = range.commonAncestorContainer;
  const element = container instanceof Element ? container : container.parentElement;
  if (element?.closest('.console__msg')) {
    return raw;
  }
  const bySeq = new Map(rows.map((row) => [String(row.line.seq), row.line]));
  const picked: string[] = [];
  for (const node of logElement.querySelectorAll<HTMLElement>('[data-seq]')) {
    const line = bySeq.get(node.dataset.seq ?? '');
    if (line && selection.containsNode(node, true)) {
      picked.push(consoleLineAsText(line));
    }
  }
  return picked.length > 0 ? picked.join('\n') : raw;
}

/** A seleção atual está dentro do log? */
function selectionInside(logElement: HTMLElement | null): boolean {
  const selection = document.getSelection();
  if (!logElement || !selection || selection.rangeCount === 0 || selection.isCollapsed) {
    return false;
  }
  return (
    selection.toString() !== '' &&
    logElement.contains(selection.getRangeAt(0).commonAncestorContainer)
  );
}

export function Console({ lines, state, label, onSave, className }: ConsoleProps): JSX.Element {
  const { t } = useTranslation('teste');
  const name = label ?? t('console.rotulo');
  const logRef = useRef<HTMLDivElement>(null);

  const [level, setLevel] = useState<LevelFilter>('all');
  const [query, setQuery] = useState('');
  // A busca em 50.000 linhas não segura a digitação.
  const deferredQuery = useDeferredValue(query);
  /** `seq` da última linha quando pausou (`-1` sem linhas); `null` acompanhando. */
  const [pausedAt, setPausedAt] = useState<number | null>(null);
  const [hasSelection, setHasSelection] = useState(false);

  // Fora do jogo aberto não há pausa: ao fechar (ou abrir de novo), a pausa some.
  const [shownState, setShownState] = useState(state);
  if (shownState !== state) {
    setShownState(state);
    if (state !== 'live') setPausedAt(null);
  }

  const live = state === 'live';
  const paused = live && pausedAt !== null;
  const following = state !== 'ended' && !paused;

  const rows = useMemo(
    () => filterLines(lines, { level, query: deferredQuery }, searchable),
    [lines, level, deferredQuery],
  );
  const filtered = level !== 'all' || deferredQuery.trim() !== '';
  const newLines = paused ? countAfter(lines, pausedAt) : 0;

  const pause = () => {
    if (live && pausedAt === null) setPausedAt(lines.at(-1)?.seq ?? -1);
  };
  const resume = () => {
    setPausedAt(null);
  };

  useEffect(() => {
    const update = () => {
      setHasSelection(selectionInside(logRef.current));
    };
    document.addEventListener('selectionchange', update);
    return () => {
      document.removeEventListener('selectionchange', update);
    };
  }, []);

  const copy = async (text: string, success: string) => {
    try {
      await navigator.clipboard.writeText(text);
      showToast({ kind: 'ok', title: success });
    } catch (cause) {
      log.warn('falha ao copiar o console', cause);
      showToast({ kind: 'danger', title: t('console.copiaFalhou') });
    }
  };
  const copyAll = () => {
    void copy(
      lines.map(consoleLineAsText).join('\n'),
      t('console.copiado', { count: lines.length }),
    );
  };
  const copySelection = () => {
    const text = logRef.current ? selectedText(logRef.current, rows) : '';
    if (text) void copy(text, t('console.selecaoCopiada'));
  };
  // Ctrl+C no log copia como "Copiar seleção" (linhas inteiras e legíveis).
  const onCopy = (event: ClipboardEvent<HTMLDivElement>) => {
    const text = selectedText(event.currentTarget, rows);
    if (text) {
      event.clipboardData.setData('text/plain', text);
      event.preventDefault();
    }
  };

  let stateView: ReactNode;
  if (state === 'waiting') {
    stateView = (
      <>
        <Loader />
        {t('console.estado.esperando')}
      </>
    );
  } else if (state === 'ended') {
    stateView = (
      <>
        <Icon icon={Square} size="sm" />
        {t('console.estado.fechado')}
      </>
    );
  } else if (paused) {
    stateView = (
      <>
        <Icon icon={Pause} size="sm" />
        {t('console.estado.pausado')}
      </>
    );
  } else {
    stateView = (
      <>
        <span className="live" aria-hidden="true" />
        {t('console.estado.aoVivo')}
      </>
    );
  }

  return (
    <section className={cn('console', className)} aria-label={name}>
      <div className="console__bar">
        <span className="console__state">{stateView}</span>
        <select
          className="select select--sm"
          aria-label={t('console.nivel.rotulo')}
          value={level}
          onChange={(event) => {
            setLevel(event.target.value as LevelFilter);
          }}
        >
          {LEVEL_OPTIONS.map(([value, key]) => (
            <option key={value} value={value}>
              {t(key)}
            </option>
          ))}
        </select>
        <div className="inputwrap">
          <Icon icon={Search} />
          <input
            className="input input--sm"
            type="search"
            value={query}
            placeholder={t('console.buscar')}
            aria-label={t('console.buscar')}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </div>
        {filtered ? (
          <span className="console__count">
            {t('console.contagem', {
              mostradas: formatInteger(rows.length),
              total: formatInteger(lines.length),
            })}
          </span>
        ) : null}
        <span className="grow" />
        {live ? (
          <Button
            size="sm"
            variant="ghost"
            icon={paused ? Play : Pause}
            onClick={paused ? resume : pause}
          >
            {paused ? t('console.retomar') : t('console.pausar')}
          </Button>
        ) : null}
        <Button
          size="sm"
          variant="ghost"
          icon={TextSelect}
          disabled={!hasSelection}
          onClick={copySelection}
        >
          {t('console.copiarSelecao')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          icon={Copy}
          disabled={lines.length === 0}
          onClick={copyAll}
        >
          {t('console.copiarTudo')}
        </Button>
        {onSave ? (
          <Button size="sm" variant="ghost" icon={Download} onClick={onSave}>
            {t('console.salvar')}
          </Button>
        ) : null}
      </div>
      <ConsoleLog
        logRef={logRef}
        name={name}
        rows={rows}
        empty={lines.length === 0 ? t('console.vazio') : t('console.semResultado')}
        query={deferredQuery}
        following={following}
        canPause={live && !paused}
        onPause={pause}
        onEnd={resume}
        onCopy={onCopy}
      />
      {paused ? (
        <div className="console__paused">
          <span>{t('console.novasLinhas', { count: newLines })}</span>
          <Button size="sm" icon={ChevronDown} onClick={resume}>
            {t('console.irParaFim')}
          </Button>
        </div>
      ) : null}
    </section>
  );
}

interface ConsoleLogProps {
  logRef: RefObject<HTMLDivElement | null>;
  name: string;
  rows: readonly ConsoleRow[];
  /** Texto quando não há linha para mostrar. */
  empty: string;
  query: string;
  /** Acompanha o fim a cada lote. */
  following: boolean;
  /** O usuário rolar para cima pausa a rolagem. */
  canPause: boolean;
  onPause: () => void;
  /** Tecla End com a rolagem pausada: volta ao fim e retoma. */
  onEnd: () => void;
  onCopy: (event: ClipboardEvent<HTMLDivElement>) => void;
}

/** O log virtualizado (linhas com quebra de texto medidas pelo navegador). */
function ConsoleLog({
  logRef,
  name,
  rows,
  empty,
  query,
  following,
  canPause,
  onPause,
  onEnd,
  onCopy,
}: ConsoleLogProps) {
  const dragging = useRef(false);
  const opened = useRef(false);

  // O TanStack Virtual devolve funções que o React Compiler não memoiza; este componente fica
  // fora da compilação, o que é o esperado para a lista virtualizada.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => logRef.current,
    estimateSize: () => ROW_ESTIMATE,
    overscan: OVERSCAN,
    paddingStart: LOG_PADDING,
    paddingEnd: LOG_PADDING,
    getItemKey: (index) => rows[index]?.line.seq ?? index,
    // Escondido (fora da tela), o navegador mede 0: a estimativa evita desenhar tudo de uma vez.
    measureElement: (element, entry, instance) =>
      measureElement(element, entry, instance) || ROW_ESTIMATE,
    // Sem medidas (testes, primeira pintura): um log de 600 px.
    initialRect: { width: 1000, height: 600 },
  });

  // Acompanha o fim a cada lote (e ao abrir, mesmo com o jogo fechado).
  const lastSeq = rows.at(-1)?.line.seq;
  useLayoutEffect(() => {
    if (rows.length === 0 || (!following && opened.current)) return;
    opened.current = true;
    virtualizer.scrollToIndex(rows.length - 1, { align: 'end' });
  }, [following, rows.length, lastSeq, virtualizer]);

  // Arrastar a barra de rolagem: o soltar pode acontecer fora do log.
  useEffect(() => {
    const release = () => {
      dragging.current = false;
    };
    window.addEventListener('pointerup', release);
    window.addEventListener('pointercancel', release);
    return () => {
      window.removeEventListener('pointerup', release);
      window.removeEventListener('pointercancel', release);
    };
  }, []);

  const scrollable = () => {
    const element = logRef.current;
    return element !== null && element.scrollHeight > element.clientHeight;
  };
  const onWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (canPause && event.deltaY < 0 && scrollable()) onPause();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (UP_KEYS.has(event.key)) {
      if (canPause && scrollable()) onPause();
    } else if (event.key === 'End') {
      onEnd();
    }
  };
  const onScroll = () => {
    const element = logRef.current;
    if (!canPause || !dragging.current || !element) return;
    const distance = element.scrollHeight - element.clientHeight - element.scrollTop;
    if (distance > END_TOLERANCE) onPause();
  };

  const items = virtualizer.getVirtualItems();
  return (
    // O log rola pelo teclado (tabIndex) e as teclas de rolar para cima pausam o acompanhamento.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <div
      ref={logRef}
      className={cn('console__log', rows.length > 0 && 'console__log--virtual')}
      role="log"
      aria-live="off"
      aria-label={name}
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      onWheel={onWheel}
      onKeyDown={onKeyDown}
      onPointerDown={() => {
        dragging.current = true;
      }}
      onScroll={onScroll}
      onCopy={onCopy}
    >
      {rows.length === 0 ? (
        <div className="console__empty">{empty}</div>
      ) : (
        <div className="console__rows" style={{ height: virtualizer.getTotalSize() }}>
          {items.map((item) => {
            const row = rows[item.index];
            return row ? (
              <LineView
                key={item.key}
                row={row}
                index={item.index}
                start={item.start}
                query={query}
                measure={virtualizer.measureElement}
              />
            ) : null;
          })}
        </div>
      )}
    </div>
  );
}

function LineView({
  row,
  index,
  start,
  query,
  measure,
}: {
  row: ConsoleRow;
  index: number;
  start: number;
  query: string;
  measure: (element: Element | null) => void;
}) {
  const { t } = useTranslation('teste');
  const { line, level } = row;
  const warden = line.origin === 'warden';
  const levelText = warden
    ? t('console.niveis.warden')
    : line.level
      ? t(`console.niveis.${line.level}`)
      : '';
  const source = warden ? t('console.origemWarden') : (line.logger ?? '');
  const text = consoleText(line);
  return (
    <div
      ref={measure}
      data-index={index}
      data-seq={line.seq}
      className={`console__line console__line--${lineTone(line, level)}`}
      style={{ transform: `translateY(${String(start)}px)` }}
    >
      <span className="console__time">{consoleTime(line)}</span>
      <span className="console__lvl">{levelText}</span>
      <span className="console__src" title={source || undefined}>
        {source}
      </span>
      <span className="console__msg">{highlight(text, query)}</span>
    </div>
  );
}

/** O texto com os trechos da busca em `<mark>`. */
function highlight(text: string, query: string): ReactNode {
  const ranges = highlightRanges(text, query);
  if (ranges.length === 0) {
    return text;
  }
  const parts: ReactNode[] = [];
  let at = 0;
  for (const [start, end] of ranges) {
    if (start > at) parts.push(text.slice(at, start));
    parts.push(<mark key={start}>{text.slice(start, end)}</mark>);
    at = end;
  }
  if (at < text.length) parts.push(text.slice(at));
  return parts;
}
