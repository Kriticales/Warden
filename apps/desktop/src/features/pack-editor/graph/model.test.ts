import { describe, expect, it } from 'vitest';

import type { FocusEdge, FocusNode, FocusReport, RelationKind } from '../../../lib/ipc/bindings';
import {
  centerOptions,
  edgeStyle,
  embeddedEntries,
  GAP,
  layoutFocus,
  ROW,
  summarizeCenter,
  wholeList,
} from './model';

function node(key: string, level: number, item = true): FocusNode {
  return {
    key,
    item: item ? { path: key, name: key } : null,
    label: key,
    level,
    embedded: [],
    library: false,
  };
}

function edge(from: string, to: string, kind: RelationKind = 'required'): FocusEdge {
  return { from, to, kind, range: null, embedded: null, note: null };
}

function report(nodes: FocusNode[], edges: FocusEdge[]): FocusReport {
  return { center: { path: 'c', name: 'c' }, depth: 2, nodes, edges, totalItems: nodes.length };
}

describe('modelo do grafo focado (D-11)', () => {
  it('cada tipo de relação tem um traço próprio', () => {
    expect(edgeStyle('required')).toBe('required');
    expect(edgeStyle('recommended')).toBe('optional');
    expect(edgeStyle('suggested')).toBe('optional');
    expect(edgeStyle('conflicts')).toBe('incompatible');
    expect(edgeStyle('inferred')).toBe('inferred');
  });

  it('colunas sempre com −1, 0 e 1; o nível 2 só quando há nós', () => {
    const one = layoutFocus(report([node('c', 0)], []));
    expect(one.columns.map((column) => column.level)).toEqual([-1, 0, 1]);
    expect(one.height).toBe(ROW);
    const two = layoutFocus(
      report(
        [node('a', -2), node('b', -1), node('c', 0), node('d', 1)],
        [edge('b', 'a'), edge('c', 'b'), edge('d', 'c')],
      ),
    );
    expect(two.columns.map((column) => column.level)).toEqual([-2, -1, 0, 1]);
    // Cada ligação entre colunas vizinhas vira uma linha na calha entre elas.
    expect(two.gutters.map((gutter) => gutter.length)).toEqual([1, 1, 1]);
  });

  it('nós centralizados na coluna e linhas ligando os centros', () => {
    const layout = layoutFocus(
      report([node('a', -1), node('b', -1), node('c', 0)], [edge('c', 'a'), edge('c', 'b')]),
    );
    expect(layout.height).toBe(2 * ROW + GAP);
    expect(layout.y.get('a')).toBe(ROW / 2);
    expect(layout.y.get('b')).toBe(ROW + GAP + ROW / 2);
    expect(layout.y.get('c')).toBe(layout.height / 2);
    expect(layout.gutters[0]?.map((line) => [line.y1, line.y2])).toEqual([
      [ROW / 2, layout.height / 2],
      [ROW + GAP + ROW / 2, layout.height / 2],
    ]);
    // O nó de fora leva o tipo da ligação com o centro.
    expect(layout.relation.get('a')?.from).toBe('c');
  });

  it('coluna com nós demais mostra o limite e conta o resto', () => {
    const nodes = [node('c', 0), ...Array.from({ length: 5 }, (_, i) => node(`u${String(i)}`, 1))];
    const layout = layoutFocus(
      report(
        nodes,
        nodes.slice(1).map((n) => edge(n.key, 'c')),
      ),
      3,
    );
    const right = layout.columns.find((column) => column.level === 1);
    expect(right?.nodes).toHaveLength(3);
    expect(right?.hidden).toBe(2);
    // Ligações com nós escondidos não são desenhadas.
    expect(layout.gutters[1]).toHaveLength(3);
  });

  it('resumo do centro separa o que exige, o opcional, o incompatível e quem precisa dele', () => {
    const summary = summarizeCenter(
      report(
        [
          node('x', -1),
          node('o', -1),
          node('falta:p', -1, false),
          node('c', 0),
          node('u', 1),
          node('r', 1),
        ],
        [
          edge('c', 'x'),
          edge('c', 'o', 'optional'),
          edge('c', 'falta:p'),
          edge('u', 'c', 'inferred'),
          edge('r', 'c', 'breaks'),
        ],
      ),
    );
    expect(summary.needs.map((n) => n.key)).toEqual(['x', 'falta:p']);
    expect(summary.optional.map((n) => n.key)).toEqual(['o']);
    expect(summary.incompatible.map((n) => n.key)).toEqual(['r']);
    expect(summary.neededBy.map((n) => n.key)).toEqual(['u']);
  });

  it('busca do centro ignora maiúsculas e ordena por nome', () => {
    const items = [{ name: 'Sodium' }, { name: 'create' }, { name: 'Ásbesto' }];
    expect(centerOptions(items, '').map((i) => i.name)).toEqual(['Ásbesto', 'create', 'Sodium']);
    expect(centerOptions(items, 'SOD').map((i) => i.name)).toEqual(['Sodium']);
  });

  it('lista do pack inteiro mostra só o que cada mod exige', () => {
    const list = wholeList(
      report([node('a', 0), node('b', 0)], [edge('a', 'b'), edge('b', 'a', 'optional')]),
    );
    expect(list.map((entry) => [entry.node.key, entry.needs])).toEqual([
      ['a', ['b']],
      ['b', []],
    ]);
  });
});

describe('embeddedEntries', () => {
  it('junta os módulos da Fabric API e mantém o resto com o nome declarado', () => {
    const entries = embeddedEntries([
      { id: 'fabric_api_base', name: 'Forgified Fabric API Base' },
      { id: 'fabric-renderer-api-v1', name: 'Fabric Renderer API (v1)' },
      { id: 'fabric-api', name: 'Fabric API' },
      { id: 'fabric-language-kotlin', name: 'Fabric Language Kotlin' },
      { id: 'sodium', name: 'Sodium' },
    ]);
    expect(entries.map((entry) => [entry.name, entry.ids, entry.fabricParts])).toEqual([
      ['Fabric API', ['fabric-api'], 0],
      ['Fabric Language Kotlin', ['fabric-language-kotlin'], 0],
      ['Sodium', ['sodium'], 0],
      ['', ['fabric_api_base', 'fabric-renderer-api-v1'], 2],
    ]);
  });
});
