// @vitest-environment node
/**
 * Pack inteiro (D-11): elementos do Cytoscape, contraste dos rótulos com os tokens do Deep Dark
 * (≥ 4,5:1) e o tempo de montar e dispor (`dagre`) um pack de 300 mods, sem canvas (headless).
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import cytoscape from 'cytoscape';
import dagre from 'cytoscape-dagre';
import { describe, expect, it } from 'vitest';

import type { FocusReport } from '../../../lib/ipc/bindings';
import { LAYOUT, LIBRARY_GROUP, wholeElements, wholeStyle, type Palette } from './whole';

cytoscape.use(dagre);

const TOKENS = readFileSync(
  join(import.meta.dirname, '..', '..', '..', '..', '..', '..', 'design', 'system', 'tokens.css'),
  'utf8',
);

function token(name: string): string {
  const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`).exec(TOKENS);
  if (!match?.[1]) throw new Error(`token ${name} não encontrado`);
  return match[1];
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((start) => {
    const channel = Number.parseInt(hex.slice(start, start + 2), 16) / 255;
    return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}

/**
 * Um pack sintético no formato dos packs reais (poucas camadas): 1 em 10 é biblioteca (algumas
 * exigem outra); cada mod exige de 1 a 3 bibliotecas e 1 em 5 também é addon de um mod grande.
 */
function syntheticPack(count: number): FocusReport {
  let seed = 7;
  const random = (limit: number) => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed % limit;
  };
  const libraries = Math.round(count / 10);
  const key = (index: number) => `mods/m${String(index)}.pw.toml`;
  const nodes = Array.from({ length: count }, (_, index) => ({
    key: key(index),
    item: { path: key(index), name: `Mod ${String(index)}` },
    label: `Mod ${String(index)}`,
    level: 0,
    embedded: [],
    library: index < libraries,
  }));
  const edges: FocusReport['edges'] = [];
  const link = (from: number, to: number, kind: 'required' | 'optional' = 'required') => {
    edges.push({ from: key(from), to: key(to), kind, range: null, embedded: null, note: null });
  };
  for (let index = 1; index < libraries; index += 1) {
    if (random(3) === 0) link(index, random(index));
  }
  for (let index = libraries; index < count; index += 1) {
    const needs = 1 + random(3);
    for (let n = 0; n < needs; n += 1)
      link(index, random(libraries), n === 2 ? 'optional' : 'required');
    if (index > libraries + 20 && random(5) === 0) link(index, libraries + random(20));
  }
  return { center: null, depth: 0, nodes, edges, totalItems: count };
}

const PALETTE: Palette = {
  text: token('--color-text'),
  text2: token('--color-text-2'),
  node: token('--color-surface-2'),
  border: token('--color-border-strong'),
  group: token('--color-bg-sunken'),
  edge: token('--color-text-3'),
  danger: token('--color-danger'),
  info: token('--color-info'),
  primary: token('--color-primary'),
  font: 'Manrope',
};

describe('pack inteiro (D-11)', () => {
  it('bibliotecas ficam agrupadas e cada ligação leva o seu traço', () => {
    const elements = wholeElements(syntheticPack(12), 'Bibliotecas') as {
      group?: string;
      data: Record<string, unknown>;
    }[];
    const group = elements.find((element) => element.data.id === LIBRARY_GROUP);
    expect(group?.data.label).toBe('Bibliotecas');
    const libraries = elements.filter((element) => element.data.parent === LIBRARY_GROUP);
    expect(libraries.map((element) => element.data.label)).toEqual(['Mod 0']);
    const styles = new Set(elements.filter((e) => e.group === 'edges').map((e) => e.data.style));
    expect(styles).toEqual(new Set(['required', 'optional']));
    // Sem bibliotecas, sem grupo.
    const plain = syntheticPack(3);
    plain.nodes.forEach((node) => {
      node.library = false;
    });
    expect(wholeElements(plain, 'Bibliotecas').some((e) => e.data.id === LIBRARY_GROUP)).toBe(
      false,
    );
  });

  it('rótulos com contraste de pelo menos 4,5:1 (tokens do Deep Dark)', () => {
    // Pack inteiro: nome do nó e rótulo do grupo das bibliotecas.
    expect(contrast(PALETTE.text, PALETTE.node)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(PALETTE.text2, PALETTE.group)).toBeGreaterThanOrEqual(4.5);
    // Grafo focado: nome e linha de baixo do nó (`.gnode`) e do centro (`.gcenter`).
    expect(contrast(token('--color-text'), token('--color-surface-2'))).toBeGreaterThanOrEqual(4.5);
    expect(contrast(token('--color-text-3'), token('--color-surface-2'))).toBeGreaterThanOrEqual(
      4.5,
    );
    expect(contrast(token('--color-text-2'), token('--color-surface-1'))).toBeGreaterThanOrEqual(
      4.5,
    );
  });

  it('300 mods: monta e dispõe com o dagre (tempo medido)', () => {
    const pack = syntheticPack(300);
    const started = performance.now();
    const cy = cytoscape({
      headless: true,
      styleEnabled: true,
      elements: wholeElements(pack, 'Bibliotecas'),
      style: wholeStyle(PALETTE),
    });
    cy.layout(LAYOUT).run();
    const elapsed = performance.now() - started;
    // Para o relatório da D-11 (`vitest run whole.node.test --silent=false`).
    // eslint-disable-next-line no-console -- o tempo medido é o resultado pedido pela D-11
    console.info(
      `[D-11] pack de 300 mods e ${String(pack.edges.length)} ligações: ${elapsed.toFixed(0)} ms`,
    );
    expect(cy.nodes().length).toBe(301); // 300 mods + o grupo das bibliotecas
    const positions = new Set(
      cy.nodes('[path]').map((n) => `${String(n.position('x'))}:${String(n.position('y'))}`),
    );
    expect(positions.size).toBe(300);
    cy.destroy();
  });
});
