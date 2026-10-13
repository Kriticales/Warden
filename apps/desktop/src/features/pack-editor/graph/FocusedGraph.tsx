/**
 * Grafo focado (SPEC T06; ARCHITECTURE §18; protótipo `depGraph`): HTML com o SVG só nas calhas
 * entre as colunas. O mod no centro; à esquerda o que ele exige (e, com 2 níveis, o que isso
 * exige); à direita quem depende dele. Cada nó do pack é um botão que abre os detalhes (T07);
 * as dependências que faltam no pack não são itens e aparecem como texto. As linhas são
 * decorativas (`aria-hidden`): as mesmas ligações estão na lista em texto ao lado.
 */
import { Box, TriangleAlert } from 'lucide-react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../components/common/PixelArt';
import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import type { FocusEdge, FocusNode, FocusReport } from '../../../lib/ipc/bindings';
import {
  DASH,
  edgeStyle,
  gutterPath,
  layoutFocus,
  ROW,
  type Column,
  type EdgeStyle,
} from './model';

/** Largura das calhas entre as colunas (a do protótipo). */
const GUTTER = 72;

export interface FocusedGraphProps {
  report: FocusReport;
  onOpen: (path: string) => void;
}

export function FocusedGraph({ report, onOpen }: FocusedGraphProps) {
  const { t } = useTranslation('grafoDesenho');
  const { t: tKind } = useTranslation('grafo');
  const layout = layoutFocus(report);
  const template = layout.columns
    .map((column) => (column.level === 0 ? 'minmax(0, 1.15fr)' : 'minmax(0, 1fr)'))
    .join(` ${String(GUTTER)}px `);
  return (
    <div
      className="graph"
      role="group"
      aria-label={t('regiao', { nome: report.center?.name ?? '' })}
      style={{ ['--g-h' as string]: `${String(layout.height)}px` }}
    >
      <div className="graph__cap" style={{ gridTemplateColumns: template }} aria-hidden="true">
        {layout.columns.map((column, index) => (
          <CaptionCell key={column.level} column={column} first={index === 0} t={t} />
        ))}
      </div>
      <div className="graph__body" style={{ gridTemplateColumns: template }}>
        {layout.columns.map((column, index) => (
          <ColumnWithGutter
            key={column.level}
            column={column}
            gutter={index > 0 ? (layout.gutters[index - 1] ?? []) : null}
            height={layout.height}
            y={layout.y}
            relation={layout.relation}
            onOpen={onOpen}
            t={t}
            tKind={tKind}
          />
        ))}
      </div>
      <Legend />
    </div>
  );
}

type DrawT = TFunction<'grafoDesenho'>;
type KindT = TFunction<'grafo'>;

function captionOf(level: number, t: DrawT): string {
  if (level === 0) return t('colunas.centro');
  if (level === -1) return t('colunas.precisaDe');
  if (level === 1) return t('colunas.usadoPor');
  return level < 0 ? t('colunas.nivel2Esquerda') : t('colunas.nivel2Direita');
}

function CaptionCell({ column, first, t }: { column: Column; first: boolean; t: DrawT }) {
  return (
    <>
      {first ? null : <span />}
      <span>{captionOf(column.level, t)}</span>
    </>
  );
}

interface ColumnProps {
  column: Column;
  gutter: { key: string; y1: number; y2: number; style: EdgeStyle }[] | null;
  height: number;
  y: Map<string, number>;
  relation: Map<string, FocusEdge>;
  onOpen: (path: string) => void;
  t: DrawT;
  tKind: KindT;
}

function ColumnWithGutter({ column, gutter, height, y, relation, onOpen, t, tKind }: ColumnProps) {
  return (
    <>
      {gutter ? (
        <svg
          className="ggutter"
          viewBox={`0 0 ${String(GUTTER)} ${String(height)}`}
          width={GUTTER}
          height={height}
          preserveAspectRatio="none"
          aria-hidden="true"
          focusable="false"
        >
          {gutter.map((line) => (
            <path
              key={line.key}
              className={`gedge gedge--${line.style}`}
              d={gutterPath(line, GUTTER)}
              strokeDasharray={DASH[line.style]}
              vectorEffect="non-scaling-stroke"
            />
          ))}
        </svg>
      ) : null}
      {column.level === 0 ? (
        <div className="gcol gcol--center" style={{ height }}>
          {column.nodes[0] ? (
            <CenterNode
              node={column.nodes[0]}
              top={y.get(column.nodes[0].key) ?? 0}
              onOpen={onOpen}
              t={t}
            />
          ) : null}
        </div>
      ) : (
        <ul className="gcol" style={{ height }}>
          {column.nodes.map((node) => (
            <li
              key={node.key}
              className="gcol__slot"
              style={{ top: (y.get(node.key) ?? 0) - ROW / 2, height: ROW }}
            >
              <GraphNode
                node={node}
                edge={relation.get(node.key)}
                onOpen={onOpen}
                t={t}
                tKind={tKind}
              />
            </li>
          ))}
          {column.hidden > 0 ? (
            <li className="gcol__more t-xs t-3">{t('no.mais', { count: column.hidden })}</li>
          ) : null}
        </ul>
      )}
    </>
  );
}

function GraphNode({
  node,
  edge,
  onOpen,
  t,
  tKind,
}: {
  node: FocusNode;
  edge: FocusEdge | undefined;
  onOpen: (path: string) => void;
  t: DrawT;
  tKind: KindT;
}) {
  const style = edge ? edgeStyle(edge.kind) : 'required';
  const sub = [
    node.item ? t('no.noPack') : t('no.falta'),
    node.library ? t('no.biblioteca') : null,
    node.embedded.length > 0 ? t('no.embutidas', { count: node.embedded.length }) : null,
  ]
    .filter(Boolean)
    .join(' · ');
  const body = (
    <>
      {style === 'incompatible' ? (
        <Icon icon={TriangleAlert} className="gnode__alert" />
      ) : (
        <NameTile seed={node.label} size="sm" />
      )}
      <span className="gnode__txt">
        <span className="gnode__name">{node.label}</span>
        <span className="gnode__sub">
          {sub}
          {edge ? <span className="gnode__kind"> · {tKind(`tipo.${edge.kind}`)}</span> : null}
        </span>
      </span>
    </>
  );
  const className = cn('gnode', `gnode--${style}`, !node.item && 'gnode--missing');
  const item = node.item;
  if (!item) {
    return <div className={className}>{body}</div>;
  }
  return (
    <button
      type="button"
      className={className}
      title={node.label}
      onClick={() => {
        onOpen(item.path);
      }}
    >
      {body}
    </button>
  );
}

function CenterNode({
  node,
  top,
  onOpen,
  t,
}: {
  node: FocusNode;
  top: number;
  onOpen: (path: string) => void;
  t: DrawT;
}) {
  const item = node.item;
  const inside = node.embedded.length > 0;
  return (
    <button
      type="button"
      className="gcenter"
      style={{ top: Math.max(0, top - (inside ? 64 : 28)) }}
      onClick={() => {
        if (item) onOpen(item.path);
      }}
    >
      <NameTile seed={node.label} size="lg" />
      <span className="gcenter__txt">
        <span className="gcenter__name">{node.label}</span>
        {node.library ? <span className="gcenter__sub">{t('no.biblioteca')}</span> : null}
        {inside ? (
          <span className="gcenter__inside">
            <Icon icon={Box} size="sm" />
            {t('no.dentroDele', {
              nomes: new Intl.ListFormat('pt-BR', { type: 'conjunction' }).format(node.embedded),
            })}
          </span>
        ) : null}
      </span>
    </button>
  );
}

function Legend() {
  const { t } = useTranslation('grafoDesenho');
  const entries: [EdgeStyle, string][] = [
    ['required', t('legenda.obrigatoria')],
    ['optional', t('legenda.opcional')],
    ['incompatible', t('legenda.incompativel')],
    ['inferred', t('legenda.inferida')],
  ];
  return (
    <ul className="graph__legend" aria-label={t('legenda.titulo')}>
      {entries.map(([style, text]) => (
        <li key={style}>
          <svg width="28" height="8" aria-hidden="true" focusable="false">
            <path className={`gedge gedge--${style}`} d="M0 4h28" strokeDasharray={DASH[style]} />
          </svg>
          {style === 'incompatible' ? <Icon icon={TriangleAlert} size="sm" /> : null}
          {text}
        </li>
      ))}
      <li>
        <Icon icon={Box} size="sm" />
        {t('legenda.embutida')}
      </li>
    </ul>
  );
}
