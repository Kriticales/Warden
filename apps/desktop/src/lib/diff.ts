/**
 * Diferença linha a linha entre dois textos curtos (trechos de config, `options.txt`), para o
 * `DiffView`. Arquivos inteiros no editor usam o `@codemirror/merge` (HANDOFF §3).
 *
 * Algoritmo: tira o começo e o fim iguais e aplica a maior subsequência comum (LCS) no meio.
 * Acima de `MAX_CELLS` comparações o meio vira "tudo removido, tudo adicionado" (correto,
 * só menos enxuto), para nunca travar a interface.
 */

/** Uma linha da diferença. Números começam em 1. */
export type DiffLine =
  | { kind: 'ctx'; oldNo: number; newNo: number; text: string }
  | { kind: 'del'; oldNo: number; text: string }
  | { kind: 'add'; newNo: number; text: string }
  /** Trecho igual escondido. */
  | { kind: 'fold'; count: number };

/** Limite de comparações da LCS (linhas do meio do antigo × do novo). */
export const MAX_CELLS = 4_000_000;

/** Quebra em linhas (`\r\n` ou `\n`); um texto vazio não tem linhas. */
export function splitLines(text: string): string[] {
  if (text === '') {
    return [];
  }
  const lines = text.split(/\r?\n/);
  if (lines.at(-1) === '') {
    lines.pop();
  }
  return lines;
}

interface Op {
  kind: 'ctx' | 'del' | 'add';
  text: string;
}

function middleOps(a: readonly string[], b: readonly string[]): Op[] {
  const n = a.length;
  const m = b.length;
  if (n === 0 || m === 0 || n * m > MAX_CELLS) {
    return [
      ...a.map((text): Op => ({ kind: 'del', text })),
      ...b.map((text): Op => ({ kind: 'add', text })),
    ];
  }
  // lcs[i * (m + 1) + j] = tamanho da LCS de a[i..] e b[j..].
  const width = m + 1;
  const lcs = new Uint32Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      lcs[i * width + j] =
        a[i] === b[j]
          ? (lcs[(i + 1) * width + j + 1] ?? 0) + 1
          : Math.max(lcs[(i + 1) * width + j] ?? 0, lcs[i * width + j + 1] ?? 0);
    }
  }
  const ops: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    const left = a[i] ?? '';
    const right = b[j] ?? '';
    if (left === right) {
      ops.push({ kind: 'ctx', text: left });
      i += 1;
      j += 1;
    } else if ((lcs[(i + 1) * width + j] ?? 0) >= (lcs[i * width + j + 1] ?? 0)) {
      ops.push({ kind: 'del', text: left });
      i += 1;
    } else {
      ops.push({ kind: 'add', text: right });
      j += 1;
    }
  }
  for (; i < n; i += 1) ops.push({ kind: 'del', text: a[i] ?? '' });
  for (; j < m; j += 1) ops.push({ kind: 'add', text: b[j] ?? '' });
  return ops;
}

function numbered(ops: readonly Op[]): Exclude<DiffLine, { kind: 'fold' }>[] {
  let oldNo = 0;
  let newNo = 0;
  return ops.map((op) => {
    switch (op.kind) {
      case 'ctx':
        oldNo += 1;
        newNo += 1;
        return { kind: 'ctx', oldNo, newNo, text: op.text };
      case 'del':
        oldNo += 1;
        return { kind: 'del', oldNo, text: op.text };
      case 'add':
        newNo += 1;
        return { kind: 'add', newNo, text: op.text };
    }
  });
}

/**
 * Esconde as linhas iguais que estão a mais de `context` linhas de uma mudança. Um trecho
 * escondido de uma linha só não vale a dobra: a linha fica visível.
 */
export function foldContext(lines: readonly DiffLine[], context: number): DiffLine[] {
  const changed = lines.map((line) => line.kind === 'add' || line.kind === 'del');
  const keep = lines.map((_, index) => {
    for (let k = Math.max(0, index - context); k <= index + context && k < lines.length; k += 1) {
      if (changed[k]) return true;
    }
    return false;
  });
  const out: DiffLine[] = [];
  let index = 0;
  while (index < lines.length) {
    if (keep[index]) {
      const line = lines[index];
      if (line) out.push(line);
      index += 1;
      continue;
    }
    let end = index;
    while (end < lines.length && !keep[end]) end += 1;
    const count = end - index;
    if (count === 1) {
      const line = lines[index];
      if (line) out.push(line);
    } else {
      out.push({ kind: 'fold', count });
    }
    index = end;
  }
  return out;
}

/** Opções de `diffLines`. */
export interface DiffOptions {
  /** Linhas iguais mostradas em volta de cada mudança; `null` mostra tudo. Padrão: 3. */
  context?: number | null;
}

/** Diferença entre `before` e `after`, linha a linha. */
export function diffLines(before: string, after: string, options: DiffOptions = {}): DiffLine[] {
  const a = splitLines(before);
  const b = splitLines(after);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start += 1;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA -= 1;
    endB -= 1;
  }
  const ops: Op[] = [
    ...a.slice(0, start).map((text): Op => ({ kind: 'ctx', text })),
    ...middleOps(a.slice(start, endA), b.slice(start, endB)),
    ...a.slice(endA).map((text): Op => ({ kind: 'ctx', text })),
  ];
  const lines = numbered(ops);
  const context = options.context === undefined ? 3 : options.context;
  return context === null ? lines : foldContext(lines, context);
}

/** Quantas linhas foram adicionadas e removidas. */
export function diffStats(lines: readonly DiffLine[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of lines) {
    if (line.kind === 'add') added += 1;
    if (line.kind === 'del') removed += 1;
  }
  return { added, removed };
}
