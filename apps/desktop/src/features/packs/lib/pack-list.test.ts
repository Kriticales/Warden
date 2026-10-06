import { describe, expect, it } from 'vitest';

import type { HygieneCause } from '../../../lib/ipc/bindings';
import { makePackRow } from '../../../test/factories';
import { causeText, filterRows, lastTestKind, loaderName, sortRows } from './pack-list';

describe('regras de Meus packs', () => {
  it('nomeia os loaders como a interface escreve', () => {
    expect(loaderName('forge')).toBe('Forge');
    expect(loaderName('neoforge')).toBe('NeoForge');
    expect(loaderName('fabric')).toBe('Fabric');
    expect(loaderName('quilt')).toBe('Quilt');
    expect(loaderName('liteloader')).toBe('LiteLoader');
    expect(loaderName('outro')).toBe('outro');
  });

  it('lê o último teste gravado pela L-04', () => {
    expect(lastTestKind(null)).toBe('never');
    expect(lastTestKind('')).toBe('never');
    expect(lastTestKind('ok')).toBe('ok');
    expect(lastTestKind('crashed')).toBe('crashed');
    expect(lastTestKind('algo-novo')).toBe('unknown');
  });

  it('ordena por alteração (mais recente primeiro, sem data no fim), nome e último teste', () => {
    const a = makePackRow({ name: 'Árvore', modifiedAtMs: 100, lastTest: 'ok' });
    const b = makePackRow({ name: 'banana', modifiedAtMs: 300, lastTest: null });
    const c = makePackRow({ name: 'Cobre 2', modifiedAtMs: null, lastTest: 'crashed' });
    const d = makePackRow({ name: 'Cobre 10', modifiedAtMs: 200, lastTest: 'ok' });
    const rows = [a, b, c, d];
    expect(sortRows(rows, 'modified').map((row) => row.name)).toEqual([
      'banana',
      'Cobre 10',
      'Árvore',
      'Cobre 2',
    ]);
    // Sem diferenciar acento e maiúscula; números em ordem natural.
    expect(sortRows(rows, 'name').map((row) => row.name)).toEqual([
      'Árvore',
      'banana',
      'Cobre 2',
      'Cobre 10',
    ]);
    expect(sortRows(rows, 'lastTest').map((row) => row.name)).toEqual([
      'Cobre 2',
      'Árvore',
      'Cobre 10',
      'banana',
    ]);
    // Não muda a lista recebida.
    expect(rows[0]).toBe(a);
  });

  it('busca por nome, versão do Minecraft e loader, sem acento nem maiúscula', () => {
    const rows = [
      makePackRow({ name: 'Vale Sereno', minecraft: '1.20.1', loader: 'forge' }),
      makePackRow({ name: 'Ação Total', minecraft: '1.21.1', loader: 'neoforge' }),
      makePackRow({ name: 'Perdido', minecraft: null, loader: null, status: 'folderMissing' }),
    ];
    expect(filterRows(rows, 'acao').map((row) => row.name)).toEqual(['Ação Total']);
    expect(filterRows(rows, '1.20').map((row) => row.name)).toEqual(['Vale Sereno']);
    expect(filterRows(rows, 'NEOFORGE').map((row) => row.name)).toEqual(['Ação Total']);
    expect(filterRows(rows, '  ')).toHaveLength(3);
    expect(filterRows(rows, 'nada')).toEqual([]);
  });

  it('dá a cada motivo da higiene uma frase do catálogo', () => {
    const pattern = (value: string, group: Extract<HygieneCause, { kind: 'pattern' }>['group']) =>
      causeText({ kind: 'pattern', pattern: value, group });
    expect(pattern('*.bak', 'anyDepth')).toEqual({ key: 'copia' });
    expect(pattern('/packwiz-installer*.jar', 'serverOnly')).toEqual({ key: 'instalador' });
    expect(pattern('/.packwiz.toml', 'serverOnly')).toEqual({ key: 'outroPrograma' });
    expect(pattern('/logs/', 'runtime')).toEqual({ key: 'log' });
    expect(pattern('/saves/', 'runtime')).toEqual({ key: 'padrao.runtime' });
    expect(pattern('/.vscode/', 'secretsAndTools')).toEqual({ key: 'padrao.secretsAndTools' });
    expect(causeText({ kind: 'unknownRootItem' })).toEqual({ key: 'raizDesconhecida' });
    expect(causeText({ kind: 'largeFile' })).toEqual({ key: 'grande' });
    expect(causeText({ kind: 'cacheLikeFolder', files: 812 })).toEqual({
      key: 'cache',
      count: 812,
    });
    expect(causeText({ kind: 'symlink' })).toEqual({ key: 'link' });
  });
});
