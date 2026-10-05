import { beforeAll, describe, expect, it } from 'vitest';

import i18n from '../../i18n';
import { makeOperation, makeProgress } from '../../test/factories';
import {
  finishedWhen,
  operationName,
  progressPercent,
  progressText,
  stageLabel,
} from './operation-text';

describe('textos de uma operação', () => {
  // Cada arquivo de teste tem o seu i18next (o Vitest isola os módulos por arquivo).
  beforeAll(() => {
    i18n.addResourceBundle('pt-BR', 'tarefas', { tipos: { pack: { export: 'Exportar' } } }, true);
    i18n.addResourceBundle('pt-BR', 'comum', { etapaTeste: { baixar: 'Baixando os mods' } }, true);
  });

  it('nome do tipo pelo catálogo, ou "Tarefa do Warden"', () => {
    expect(operationName('pack.export')).toBe('Exportar');
    expect(operationName('nao.existe')).toBe('Tarefa do Warden');
  });

  it('etapa de qualquer catálogo, ou nada', () => {
    expect(stageLabel(makeOperation({ stage: { id: 'b', labelKey: 'etapaTeste.baixar' } }))).toBe(
      'Baixando os mods',
    );
    expect(
      stageLabel(makeOperation({ stage: { id: 'b', labelKey: 'comum:etapaTeste.baixar' } })),
    ).toBe('Baixando os mods');
    expect(stageLabel(makeOperation({ stage: { id: 'x', labelKey: 'sem.texto' } }))).toBeNull();
    expect(stageLabel(makeOperation())).toBeNull();
  });
});

describe('progresso', () => {
  it('itens e bytes, com e sem total', () => {
    expect(progressText(makeProgress(12, 128))).toBe('12 de 128');
    expect(progressText(makeProgress(1204, null))).toBe('1.204');
    expect(progressText(makeProgress(160 * 1024 * 1024, 420 * 1024 * 1024, 'bytes'))).toBe(
      '160,0 MB de 420,0 MB',
    );
  });

  it('porcentagem só com total conhecido e positivo', () => {
    expect(progressPercent(makeProgress(1, 4))).toBe(25);
    expect(progressPercent(makeProgress(9, 4))).toBe(100);
    expect(progressPercent(makeProgress(1, null))).toBeNull();
    expect(progressPercent(makeProgress(0, 0))).toBeNull();
    expect(progressPercent(null)).toBeNull();
  });
});

describe('quando terminou', () => {
  it('hoje, ontem e outro dia', () => {
    const now = new Date(2026, 9, 4, 15, 0).getTime();
    expect(finishedWhen(new Date(2026, 9, 4, 14, 1).getTime(), now)).toBe('14:01');
    expect(finishedWhen(new Date(2026, 9, 3, 21, 40).getTime(), now)).toBe('ontem, 21:40');
    expect(finishedWhen(new Date(2026, 8, 28, 10, 12).getTime(), now)).toBe('28/09/2026, 10:12');
  });
});
