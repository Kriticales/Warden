/**
 * Elementos e estilo do pack inteiro para o Cytoscape (D-11). Puro: testado sem canvas.
 * As bibliotecas ficam agrupadas num nó composto ("Bibliotecas"); cada ligação tem o traço do
 * seu tipo, como no grafo focado (cheia, tracejada, vermelha, pontilhada).
 */
import type { ElementDefinition, StylesheetJson } from 'cytoscape';
import type { DagreLayoutOptions } from 'cytoscape-dagre';

import type { FocusReport } from '../../../lib/ipc/bindings';
import { edgeStyle } from './model';

/** Id do nó composto que agrupa as bibliotecas. */
export const LIBRARY_GROUP = 'grupo:bibliotecas';

/** Camadas da esquerda (quem exige) para a direita (quem é exigido). */
export const LAYOUT: DagreLayoutOptions = {
  name: 'dagre',
  rankDir: 'LR',
  nodeSep: 12,
  rankSep: 64,
};

/** Cores do tema lidas dos tokens. */
export interface Palette {
  text: string;
  text2: string;
  node: string;
  border: string;
  group: string;
  edge: string;
  danger: string;
  info: string;
  primary: string;
  /** Fonte da interface (`--font-ui`). */
  font: string;
}

export function wholeElements(report: FocusReport, librariesLabel: string): ElementDefinition[] {
  const hasLibraries = report.nodes.some((node) => node.library);
  const elements: ElementDefinition[] = [];
  if (hasLibraries) {
    elements.push({ group: 'nodes', data: { id: LIBRARY_GROUP, label: librariesLabel } });
  }
  for (const node of report.nodes) {
    elements.push({
      group: 'nodes',
      data: {
        id: node.key,
        label: node.label,
        path: node.item?.path ?? null,
        ...(node.library ? { parent: LIBRARY_GROUP } : {}),
      },
    });
  }
  report.edges.forEach((edge, index) => {
    elements.push({
      group: 'edges',
      data: {
        id: `e${String(index)}`,
        source: edge.from,
        target: edge.to,
        style: edgeStyle(edge.kind),
      },
    });
  });
  return elements;
}

export function wholeStyle(colors: Palette): StylesheetJson {
  return [
    {
      selector: 'node',
      style: {
        shape: 'rectangle',
        'background-color': colors.node,
        'border-width': 1,
        'border-color': colors.border,
        label: 'data(label)',
        color: colors.text,
        'font-family': colors.font,
        'font-size': 11,
        'text-valign': 'center',
        'text-halign': 'center',
        'text-max-width': '140px',
        'text-wrap': 'ellipsis',
        width: 150,
        height: 28,
      },
    },
    {
      selector: ':parent',
      style: {
        'background-color': colors.group,
        'border-color': colors.border,
        'border-style': 'dashed',
        color: colors.text2,
        'text-valign': 'top',
        'text-halign': 'center',
        'font-size': 12,
        padding: '12px',
      },
    },
    {
      selector: 'node:selected',
      style: { 'border-color': colors.primary, 'border-width': 2 },
    },
    {
      selector: 'edge',
      style: {
        width: 1.5,
        'line-color': colors.edge,
        'target-arrow-color': colors.edge,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.8,
        'curve-style': 'bezier',
      },
    },
    { selector: 'edge[style = "optional"]', style: { 'line-style': 'dashed' } },
    {
      selector: 'edge[style = "incompatible"]',
      style: { 'line-color': colors.danger, 'target-arrow-color': colors.danger },
    },
    {
      selector: 'edge[style = "inferred"]',
      style: {
        'line-style': 'dotted',
        'line-color': colors.info,
        'target-arrow-color': colors.info,
      },
    },
  ];
}
