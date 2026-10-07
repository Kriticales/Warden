import { describe, expect, it } from 'vitest';

import { ALFA, available, BETA, FIXADO, GAMA, makeReport, updateItem } from './testing';
import { countUpdates, effectiveStatus, reportItem, updatableItems } from './model';
import { makeItem } from '../pack-editor/editor.fixtures';

describe('estado das atualizações por linha', () => {
  const report = makeReport([
    available(ALFA, '1.1'),
    updateItem(BETA),
    updateItem(GAMA, { status: 'notChecked', reason: 'keyMissing' }),
    updateItem(FIXADO, { status: 'available', newVersion: null }),
  ]);

  it('o resultado do relatório vale enquanto a versão instalada for a da verificação', () => {
    expect(reportItem(report, ALFA)?.status).toBe('available');
    const moved = { ...ALFA, sourceVersionId: 'outra' };
    expect(reportItem(report, moved)).toBeNull();
    expect(effectiveStatus(report, moved)).toBe('notChecked');
  });

  it('sem relatório, nada foi verificado; fixado, local e link ficam fora', () => {
    expect(effectiveStatus(null, ALFA)).toBe('notChecked');
    expect(effectiveStatus(report, FIXADO)).toBe('pinned');
    expect(effectiveStatus(report, makeItem({ source: 'local' }))).toBe('notApplicable');
    expect(effectiveStatus(report, makeItem({ source: 'url' }))).toBe('notApplicable');
    expect(effectiveStatus(report, makeItem({ state: 'invalid' }))).toBe('notApplicable');
  });

  it('só quem tem atualização disponível pode ser atualizado; fixados nunca entram', () => {
    expect(updatableItems(report, [ALFA, BETA, GAMA, FIXADO]).map((i) => i.name)).toEqual(['Alfa']);
  });

  it('conta atualizações, falhas e problemas de chave separados', () => {
    const failed = makeReport([
      available(ALFA, '1.1'),
      updateItem(BETA, { status: 'failed', reason: 'offline' }),
      updateItem(GAMA, { status: 'notChecked', reason: 'keyInvalid' }),
    ]);
    expect(countUpdates(failed, [ALFA, BETA, GAMA])).toEqual({
      available: 1,
      failed: 1,
      keyProblem: 1,
    });
  });
});
