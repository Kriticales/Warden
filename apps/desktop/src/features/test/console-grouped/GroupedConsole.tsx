/**
 * Console "Agrupado por mod" e "Só problemas" (SPEC T13; ARCHITECTURE §9.8; protótipo
 * `consoleGroups` do design system; ROADMAP D-09).
 *
 * Os grupos e os itens vêm prontos do Rust (`console_grouped`): uma linha por mod (com "Ver no
 * pack"), "Jogo e loader" e "Desconhecido"; dentro de cada grupo aberto, as linhas repetidas
 * viram um item "×N" com a primeira e a última hora, e o stack trace é um item recolhido com a
 * primeira linha de mod em destaque. "Só problemas" mostra os erros, avisos e stack traces que
 * não são ruído conhecido. A lista é virtualizada e cada grupo traz no máximo [`PAGE`] itens
 * por vez ("Mostrar mais"); com o jogo aberto, tudo é relido a cada segundo. O nome e o link de
 * cada mod vêm do inventário do pack, que se atualiza quando o pack muda.
 */
import { useNavigate } from '@tanstack/react-router';
import { measureElement, useVirtualizer } from '@tanstack/react-virtual';
import { ChevronDown, ChevronRight, Copy, ExternalLink } from 'lucide-react';
import { useMemo, useRef, useState, type JSX } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Loader } from '../../../components/ui/loader';
import { showToast } from '../../../components/ui/toast';
import { cn } from '../../../lib/cn';
import { formatInteger } from '../../../lib/format';
import type { GroupRow, GroupedRequest, ItemView, PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { log } from '../../../lib/log';
import { useInventory } from '../../pack-editor/api';
import { consoleLineAsText, consoleTime } from '../text';
import { groupLines, useGroupedConsole, useItemLines } from './api';

import './grouped.css';

/** Itens por vez em cada grupo aberto e em "Só problemas". */
export const PAGE = 200;
/** Altura estimada de uma linha (12 px × 1,65) e de um cabeçalho de grupo. */
const ROW_ESTIMATE = 20;
const HEAD_ESTIMATE = 36;

export type GroupedMode = 'grouped' | 'problems';

export interface GroupedConsoleProps {
  packId: PackId;
  /** A sessão (a do jogo aberto ou uma gravada); `null` antes de o jogo abrir. */
  sessionId: string | null;
  mode: GroupedMode;
  /** Busca do console (já adiada pela digitação). */
  query: string;
  /** Jogo aberto: relê a cada segundo. */
  live: boolean;
}

type Row =
  | { kind: 'group'; row: GroupRow; open: boolean }
  | { kind: 'item'; item: ItemView; group: string | null }
  | { kind: 'more'; key: string | null; remaining: number };

export function GroupedConsole({
  packId,
  sessionId,
  mode,
  query,
  live,
}: GroupedConsoleProps): JSX.Element {
  const { t } = useTranslation('console');
  const [open, setOpen] = useState<Record<string, number>>({});
  const [problemsLimit, setProblemsLimit] = useState(PAGE);
  const request: GroupedRequest = useMemo(
    () =>
      mode === 'grouped'
        ? {
            mode: 'byMod',
            open: Object.entries(open).map(([key, limit]) => ({ key, limit })),
            limit: 0,
            query,
          }
        : { mode: 'problems', open: [], limit: problemsLimit, query },
    [mode, open, problemsLimit, query],
  );
  const view = useGroupedConsole(packId, sessionId, request, live);
  const inventory = useInventory(packId);
  // Nome atual de cada item do pack (o inventário é relido quando o pack muda).
  const names = useMemo(
    () => new Map((inventory.data?.items ?? []).map((item) => [item.path, item.name])),
    [inventory.data],
  );

  const groupName = (row: GroupRow): string => {
    const { group } = row;
    switch (group.kind) {
      case 'game':
        return t('grupo.jogo');
      case 'unknown':
        return t('grupo.desconhecido');
      case 'item':
        return (group.item ? names.get(group.item) : undefined) ?? row.name ?? group.modId ?? '';
      case 'mod':
        return row.name ?? group.modId ?? '';
    }
  };

  const data = view.data;
  const rows: Row[] = useMemo(() => {
    if (!data) return [];
    if (mode === 'problems') {
      const out: Row[] = data.problems.map((item) => ({ kind: 'item', item, group: item.group }));
      const remaining = data.problemsTotal - data.problems.length;
      if (remaining > 0) out.push({ kind: 'more', key: null, remaining });
      return out;
    }
    const out: Row[] = [];
    for (const row of data.groups) {
      const isOpen = open[row.group.key] !== undefined;
      out.push({ kind: 'group', row, open: isOpen });
      if (!isOpen) continue;
      for (const item of row.items) out.push({ kind: 'item', item, group: null });
      const remaining = row.group.items - row.items.length;
      if (remaining > 0) out.push({ kind: 'more', key: row.group.key, remaining });
    }
    return out;
  }, [data, mode, open]);

  // Nome do grupo de cada item em "Só problemas".
  const problemGroup = (key: string): string => {
    if (key === 'game') return t('grupo.jogo');
    if (key === 'unknown') return t('grupo.desconhecido');
    const item = key.startsWith('item:') ? key.slice('item:'.length) : null;
    return (item ? names.get(item) : undefined) ?? data?.names[key] ?? key.replace(/^mod:/, '');
  };

  const toggle = (key: string) => {
    setOpen((current) =>
      current[key] === undefined
        ? { ...current, [key]: PAGE }
        : Object.fromEntries(Object.entries(current).filter(([open]) => open !== key)),
    );
  };
  const more = (key: string | null) => {
    if (key === null) {
      setProblemsLimit((limit) => limit + PAGE);
    } else {
      setOpen((current) => ({ ...current, [key]: (current[key] ?? 0) + PAGE }));
    }
  };

  const label = mode === 'grouped' ? t('rotulo.agrupado') : t('rotulo.problemas');
  let empty: string | null = null;
  if (sessionId === null) empty = t('esperando');
  else if (view.isError) empty = t('erro');
  else if (data && rows.length === 0) {
    if (query.trim() !== '') empty = t('semResultado');
    else empty = mode === 'problems' ? t('semProblemas') : t('vazio');
  }

  return (
    <GroupedList
      label={label}
      rows={rows}
      empty={empty}
      loading={sessionId !== null && !data && !view.isError}
      renderRow={(row) => {
        switch (row.kind) {
          case 'group':
            return (
              <GroupHead
                packId={packId}
                sessionId={sessionId ?? ''}
                row={row.row}
                name={groupName(row.row)}
                open={row.open}
                inPack={row.row.group.item !== null && names.has(row.row.group.item)}
                onToggle={() => {
                  toggle(row.row.group.key);
                }}
              />
            );
          case 'item':
            return (
              <ItemLine
                packId={packId}
                sessionId={sessionId ?? ''}
                item={row.item}
                group={row.group === null ? null : problemGroup(row.group)}
              />
            );
          case 'more':
            return (
              <div className="cline cline--more">
                <span />
                <span />
                <span>
                  <Button
                    size="sm"
                    variant="link"
                    onClick={() => {
                      more(row.key);
                    }}
                  >
                    {t('mostrarMais', { count: row.remaining, n: formatInteger(row.remaining) })}
                  </Button>
                </span>
              </div>
            );
        }
      }}
      rowKey={(row) => {
        switch (row.kind) {
          case 'group':
            return `g:${row.row.group.key}`;
          case 'item':
            return `i:${String(row.item.id)}`;
          case 'more':
            return `m:${row.key ?? ''}`;
        }
      }}
    />
  );
}

interface GroupedListProps {
  label: string;
  rows: readonly Row[];
  empty: string | null;
  loading: boolean;
  renderRow: (row: Row) => JSX.Element;
  rowKey: (row: Row) => string;
}

/** A lista virtualizada (linhas com altura medida pelo navegador). */
function GroupedList({ label, rows, empty, loading, renderRow, rowKey }: GroupedListProps) {
  const { t } = useTranslation('console');
  const scrollRef = useRef<HTMLDivElement>(null);
  // O TanStack Virtual devolve funções que o React Compiler não memoiza; este componente fica
  // fora da compilação, o que é o esperado para a lista virtualizada.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => (rows[index]?.kind === 'group' ? HEAD_ESTIMATE : ROW_ESTIMATE),
    overscan: 12,
    getItemKey: (index) => {
      const row = rows[index];
      return row ? rowKey(row) : index;
    },
    // Escondido (fora da tela), o navegador mede 0: a estimativa evita desenhar tudo de uma vez.
    measureElement: (element, entry, instance) =>
      measureElement(element, entry, instance) || ROW_ESTIMATE,
    initialRect: { width: 1000, height: 600 },
  });

  let body: JSX.Element;
  if (empty !== null) {
    body = <div className="console__empty">{empty}</div>;
  } else if (loading) {
    body = (
      <div className="console__empty">
        <Loader />
        {t('carregando')}
      </div>
    );
  } else {
    body = (
      <div className="cgrouped__rows" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((virtual) => {
          const row = rows[virtual.index];
          return row ? (
            <div
              key={virtual.key}
              ref={virtualizer.measureElement}
              data-index={virtual.index}
              className="cgrouped__row"
              style={{ transform: `translateY(${String(virtual.start)}px)` }}
            >
              {renderRow(row)}
            </div>
          ) : null;
        })}
      </div>
    );
  }
  return (
    <div
      ref={scrollRef}
      className="console__log console__log--grouped"
      role="region"
      aria-label={label}
      aria-busy={loading}
      // A região rola pelo teclado.
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
    >
      {body}
    </div>
  );
}

function GroupHead({
  packId,
  sessionId,
  row,
  name,
  open,
  inPack,
  onToggle,
}: {
  packId: PackId;
  sessionId: string;
  row: GroupRow;
  name: string;
  open: boolean;
  /** O item ainda está no pack (link "Ver no pack"). */
  inPack: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation('console');
  const navigate = useNavigate();
  const [copying, setCopying] = useState(false);
  const { group } = row;
  const itemPath = group.item;

  const copy = async () => {
    setCopying(true);
    try {
      const lines = await groupLines(packId, sessionId, group.key);
      await navigator.clipboard.writeText(lines.map(consoleLineAsText).join('\n'));
      showToast({
        kind: 'ok',
        title: t('copiado', { count: lines.length, n: formatInteger(lines.length) }),
      });
    } catch (cause) {
      log.warn('falha ao copiar as linhas do grupo', cause);
      showToast({
        kind: 'danger',
        title: t('copiaFalhou'),
        ...(isAppError(cause) ? { text: appErrorMessage(toAppError(cause)) } : {}),
      });
    } finally {
      setCopying(false);
    }
  };

  return (
    <div className="cgroup__head">
      <button
        type="button"
        className="cgroup__toggle"
        aria-expanded={open}
        aria-label={open ? t('grupo.fechar', { nome: name }) : t('grupo.abrir', { nome: name })}
        onClick={onToggle}
      >
        <Icon icon={open ? ChevronDown : ChevronRight} size="sm" />
        <span className="cgroup__mod">{name}</span>
      </button>
      <span className="cgroup__counts">
        {group.errors > 0 ? (
          <span className="t-danger">
            {t('contagem.erros', { count: group.errors, n: formatInteger(group.errors) })}
          </span>
        ) : null}
        {group.warnings > 0 ? (
          <span className="t-warn">
            {t('contagem.avisos', { count: group.warnings, n: formatInteger(group.warnings) })}
          </span>
        ) : null}
        {group.repeated > 0 ? (
          <span>
            {t('contagem.repetidas', { count: group.repeated, n: formatInteger(group.repeated) })}
          </span>
        ) : null}
        <span>
          {t('contagem.linhas', { count: group.entries, n: formatInteger(group.entries) })}
        </span>
      </span>
      {inPack && itemPath !== null ? (
        <Button
          size="sm"
          variant="ghost"
          icon={ExternalLink}
          aria-label={t('verNoPackDe', { nome: name })}
          onClick={() => {
            void navigate({
              to: '/packs/$packId/mods',
              params: { packId },
              search: { item: itemPath },
            });
          }}
        >
          {t('verNoPack')}
        </Button>
      ) : null}
      <Button
        size="sm"
        variant="ghost"
        icon={Copy}
        loading={copying}
        aria-label={t('copiarDe', { nome: name })}
        onClick={() => {
          void copy();
        }}
      >
        {t('copiar')}
      </Button>
    </div>
  );
}

function isAppError(value: unknown): boolean {
  return typeof value === 'object' && value !== null && 'code' in value;
}

const SEVERITY_CLASS = { error: 'cline--error', warning: 'cline--warn', info: '' } as const;

function ItemLine({
  packId,
  sessionId,
  item,
  group,
}: {
  packId: PackId;
  sessionId: string;
  item: ItemView;
  /** Nome do grupo, em "Só problemas". */
  group: string | null;
}) {
  const { t } = useTranslation(['console', 'teste']);
  const [expanded, setExpanded] = useState(false);
  const levelText = item.level
    ? t(`teste:console.niveis.${item.level}`)
    : item.severity === 'error'
      ? t('teste:console.niveis.error')
      : '';
  const vezes = formatInteger(item.count);
  const times =
    item.count > 1
      ? t('console:primeiraUltima', {
          primeira: consoleTime(item.first),
          ultima: consoleTime(item.last),
        })
      : consoleTime(item.first);
  return (
    <div className={cn('cline', SEVERITY_CLASS[item.severity], item.trace && 'cline--stack')}>
      <span className="cline__rep">
        {item.count > 1 ? (
          <span title={t('console:repeticoesRotulo', { vezes })}>
            {t('console:repeticoes', { vezes })}
          </span>
        ) : null}
      </span>
      <span className="cline__lvl">{levelText}</span>
      <div className="cline__msg">
        {group !== null ? <span className="cline__group">{group}</span> : null}
        <span>{item.text}</span>
        {item.noise ? (
          <span className="tag tag--plain cline__noise" title={t('console:ruidoDica')}>
            {t('console:ruido')}
          </span>
        ) : null}
        {times ? <span className="cline__time"> {times}</span> : null}
        {item.extraLines > 0 ? (
          <div className="cstack">
            {!expanded && item.modFrame ? (
              <div className="cstack__f cstack__f--mod">
                <span className="sr-only">{t('console:primeiraLinhaDeMod')} </span>
                {item.modFrame}
              </div>
            ) : null}
            <button
              type="button"
              className="cstack__toggle"
              aria-expanded={expanded}
              onClick={() => {
                setExpanded((value) => !value);
              }}
            >
              <Icon icon={expanded ? ChevronDown : ChevronRight} size="sm" />
              {item.trace
                ? t('console:stack', { count: item.extraLines, n: formatInteger(item.extraLines) })
                : t('console:maisLinhas', {
                    count: item.extraLines,
                    n: formatInteger(item.extraLines),
                  })}
            </button>
            {expanded ? (
              <ItemDetail
                packId={packId}
                sessionId={sessionId}
                id={item.id}
                modFrame={item.modFrame}
              />
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** As linhas do item aberto (a primeira vez que apareceu), com a primeira linha de mod. */
function ItemDetail({
  packId,
  sessionId,
  id,
  modFrame,
}: {
  packId: PackId;
  sessionId: string;
  id: number;
  modFrame: string | null;
}) {
  const { t } = useTranslation('console');
  const lines = useItemLines(packId, sessionId, id, true);
  if (!lines.data) {
    return <div className="cstack__more">{lines.isError ? t('erro') : t('lendoStack')}</div>;
  }
  const rest = (lines.data[0]?.text ?? '').split('\n').slice(1);
  return (
    <div>
      {rest.map((line, index) => {
        const mod = modFrame !== null && line.trim() === modFrame;
        return (
          // As linhas não mudam de ordem nem de conteúdo: a posição é a chave.
          <div key={index} className={cn('cstack__f', mod && 'cstack__f--mod')}>
            {mod ? <span className="sr-only">{t('primeiraLinhaDeMod')} </span> : null}
            {line}
          </div>
        );
      })}
    </div>
  );
}
