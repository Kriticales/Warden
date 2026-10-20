/**
 * Todo tipo de operação que o backend registra (`OperationKind::new("...")` em `src-tauri/src`)
 * tem nome em `tarefas.ts`: sem nome, a gaveta e o toast mostram só "Tarefa do Warden". Os
 * arquivos de teste do backend, os tipos de teste (`teste.*`) e o recuo `unknown` ficam de fora.
 */
import { describe, expect, it } from 'vitest';

import { operationName } from './operation-text';

const SOURCES = import.meta.glob<string>('../../../src-tauri/src/**/*.rs', {
  query: '?raw',
  import: 'default',
  eager: true,
});
const KIND = /OperationKind::new\(\s*"([^"]+)"\s*\)/g;
const TEST_FILE = /\/tests?(\.rs|\/)|_tests\.rs$/;

/** Tipos dos arquivos do app, sem os de teste. */
function backendKinds(): string[] {
  const kinds = new Set<string>();
  for (const [file, text] of Object.entries(SOURCES)) {
    if (TEST_FILE.test(file.slice(file.indexOf('/src/')))) {
      continue;
    }
    for (const match of text.matchAll(KIND)) {
      const kind = match[1] ?? '';
      if (kind !== 'unknown' && !kind.startsWith('teste.')) {
        kinds.add(kind);
      }
    }
  }
  return [...kinds].sort();
}

describe('tipos de operação', () => {
  it('todo tipo registrado pelo backend tem nome em tarefas.ts', () => {
    const kinds = backendKinds();
    expect(kinds).toContain('diagnostics.check');
    expect(kinds.length).toBeGreaterThan(30);
    expect(kinds.filter((kind) => operationName(kind) === 'Tarefa do Warden')).toEqual([]);
  });
});
