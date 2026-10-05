import { describe, expect, it } from 'vitest';

import { diffLines, diffStats, foldContext, MAX_CELLS, splitLines, type DiffLine } from './diff';

/** Reconstrói os dois textos a partir da diferença completa (sem dobras). */
function rebuild(lines: readonly DiffLine[]): { before: string[]; after: string[] } {
  const before: string[] = [];
  const after: string[] = [];
  for (const line of lines) {
    if (line.kind === 'ctx') {
      before.push(line.text);
      after.push(line.text);
    } else if (line.kind === 'del') {
      before.push(line.text);
    } else if (line.kind === 'add') {
      after.push(line.text);
    }
  }
  return { before, after };
}

/** Gerador pseudoaleatório determinístico (mulberry32): o teste é sempre o mesmo. */
function random(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

describe('splitLines', () => {
  it('aceita \\n e \\r\\n e ignora a quebra final', () => {
    expect(splitLines('')).toEqual([]);
    expect(splitLines('a\r\nb\n')).toEqual(['a', 'b']);
    expect(splitLines('a\n\nb')).toEqual(['a', '', 'b']);
  });
});

describe('diffLines', () => {
  it('troca de um valor numa config', () => {
    const before = 'a=1\nb=2\nc=3\n';
    const after = 'a=1\nb=5\nc=3\n';
    expect(diffLines(before, after, { context: null })).toEqual([
      { kind: 'ctx', oldNo: 1, newNo: 1, text: 'a=1' },
      { kind: 'del', oldNo: 2, text: 'b=2' },
      { kind: 'add', newNo: 2, text: 'b=5' },
      { kind: 'ctx', oldNo: 3, newNo: 3, text: 'c=3' },
    ]);
  });

  it('textos iguais viram uma dobra só; vazio contra texto é tudo adicionado', () => {
    expect(diffLines('x\ny\nz\nw\nv\n', 'x\ny\nz\nw\nv\n', { context: 1 })).toEqual([
      { kind: 'fold', count: 5 },
    ]);
    expect(diffStats(diffLines('', 'a\nb'))).toEqual({ added: 2, removed: 0 });
    expect(diffStats(diffLines('a\nb', ''))).toEqual({ added: 0, removed: 2 });
  });

  it('dobra o que fica longe das mudanças, mas não dobra uma linha só', () => {
    const before = Array.from({ length: 12 }, (_, i) => `l${String(i)}`).join('\n');
    const after = before.replace('l6', 'L6');
    const folded = diffLines(before, after, { context: 2 });
    expect(folded[0]).toEqual({ kind: 'fold', count: 4 });
    expect(folded.at(-1)).toEqual({ kind: 'fold', count: 3 });
    const single = foldContext(diffLines('a\nb\nc', 'a\nB\nc', { context: null }), 0);
    expect(single.map((line) => line.kind)).toEqual(['ctx', 'del', 'add', 'ctx']);
  });

  it('acima do limite de comparações continua correto (tudo removido, tudo adicionado)', () => {
    const size = Math.ceil(Math.sqrt(MAX_CELLS)) + 1;
    const before = Array.from({ length: size }, (_, i) => `a${String(i)}`).join('\n');
    const after = Array.from({ length: size }, (_, i) => `b${String(i)}`).join('\n');
    const lines = diffLines(before, after, { context: null });
    expect(diffStats(lines)).toEqual({ added: size, removed: size });
    expect(rebuild(lines)).toEqual({ before: splitLines(before), after: splitLines(after) });
  });

  it('propriedade: 500 pares aleatórios reconstroem os dois textos e a numeração é contínua', () => {
    const next = random(20261004);
    const alphabet = ['a', 'b', 'c', 'd', '', 'chave=1', 'chave=2'];
    const pick = () => alphabet[Math.floor(next() * alphabet.length)] ?? '';
    for (let round = 0; round < 500; round += 1) {
      const before = Array.from({ length: Math.floor(next() * 15) }, pick);
      const after = Array.from({ length: Math.floor(next() * 15) }, pick);
      const lines = diffLines(before.join('\n'), after.join('\n'), { context: null });
      const rebuilt = rebuild(lines);
      expect(rebuilt.before).toEqual(splitLines(before.join('\n')));
      expect(rebuilt.after).toEqual(splitLines(after.join('\n')));
      let oldNo = 0;
      let newNo = 0;
      for (const line of lines) {
        if (line.kind === 'ctx' || line.kind === 'del') {
          oldNo += 1;
          expect(line.oldNo).toBe(oldNo);
        }
        if (line.kind === 'ctx' || line.kind === 'add') {
          newNo += 1;
          expect(line.newNo).toBe(newNo);
        }
      }
      // A LCS nunca é pior que "nada em comum": mudanças ≤ soma dos tamanhos.
      const { added, removed } = diffStats(lines);
      expect(added + removed).toBeLessThanOrEqual(rebuilt.before.length + rebuilt.after.length);
      expect(rebuilt.before.length - removed).toBe(rebuilt.after.length - added);
    }
  });
});
