/**
 * Partes puras do Testar: o estado do teste pelos eventos do canal, as etapas do indicador e os
 * textos (durações, memória, mensagens do Warden no console).
 */
import { describe, expect, it } from 'vitest';

import { DEFAULT_REQUEST, MAX_CONSOLE_LINES, appendLines, applyEvent, type TestRun } from './store';
import { currentStep, orderedSteps } from './steps';
import { gameLine, wardenLine } from './test.fixtures';
import { consoleLineAsText, consoleText, formatDuration, formatMemory, stageText } from './text';

function run(overrides: Partial<TestRun> = {}): TestRun {
  return {
    packId: '01J9ZQ0000000000000000000A',
    status: 'preparing',
    request: DEFAULT_REQUEST,
    operationId: null,
    stage: null,
    progress: null,
    stagesSeen: [],
    lines: [],
    warnings: [],
    error: null,
    result: null,
    startedAtMs: 0,
    stopping: false,
    adopted: false,
    ...overrides,
  };
}

describe('estado do teste pelo canal', () => {
  it('etapa zera o progresso e entra uma vez na lista das vistas', () => {
    let state = applyEvent(run(), {
      type: 'stage',
      stage: 'java.download',
      labelKey: 'java:etapa.download',
    });
    state = applyEvent(state, { type: 'progress', current: 5, total: 10, unit: 'bytes' });
    expect(state.progress).toEqual({ current: 5, total: 10, unit: 'bytes' });
    state = applyEvent(state, {
      type: 'stage',
      stage: 'syncPack',
      labelKey: 'test.stage.syncPack',
    });
    state = applyEvent(state, {
      type: 'stage',
      stage: 'syncPack',
      labelKey: 'test.stage.syncPack',
    });
    expect(state.progress).toBeNull();
    expect(state.stagesSeen).toEqual(['java.download', 'syncPack']);
  });

  it('a primeira linha do console marca o jogo como aberto; avisos são guardados', () => {
    const warning = {
      code: { domain: 'app', code: 'TEST_MEMORY_HIGH_JAVA8' },
      params: { memoryMb: '10240' },
      detail: null,
      retryable: false,
      operationId: null,
    } as const;
    let state = applyEvent(run(), { type: 'warning', error: warning });
    state = applyEvent(state, { type: 'console', lines: [gameLine('a')] });
    expect(state.status).toBe('running');
    expect(state.warnings).toEqual([warning]);
    expect(state.lines).toHaveLength(1);
  });

  it('o console guarda só as últimas 50.000 linhas', () => {
    const many = Array.from({ length: MAX_CONSOLE_LINES }, (_, seq) =>
      gameLine(String(seq), { seq }),
    );
    const joined = appendLines(many, [gameLine('nova', { seq: MAX_CONSOLE_LINES })]);
    expect(joined).toHaveLength(MAX_CONSOLE_LINES);
    expect(joined[0]?.text).toBe('1');
    expect(joined.at(-1)?.text).toBe('nova');
  });
});

describe('indicador de etapas', () => {
  it('avança pelas etapas do backend e nunca volta; com o jogo aberto, tudo concluído', () => {
    const steps = orderedSteps();
    expect(steps.map((step) => step.id)).toEqual(['prepare', 'sync', 'launch']);
    expect(currentStep([], false)).toBe(0);
    expect(currentStep(['test.java', 'launcher.download'], false)).toBe(0);
    expect(currentStep(['launcher.download', 'syncPack'], false)).toBe(1);
    expect(currentStep(['syncPack', 'test.launch', 'java.resolve'], false)).toBe(2);
    expect(currentStep(['syncPack'], true)).toBe(steps.length);
  });
});

describe('textos', () => {
  it('durações e memória', () => {
    expect(formatDuration(48_000)).toBe('48 s');
    expect(formatDuration(102_000)).toBe('1 min 42 s');
    expect(formatDuration(12 * 60_000 + 5_000)).toBe('12 min');
    expect(formatDuration(65 * 60_000)).toBe('1 h 5 min');
    expect(formatDuration(3_600_000)).toBe('1 h');
    expect(formatMemory(6144)).toBe('6 GB');
    expect(formatMemory(1536)).toBe('1,5 GB');
    expect(formatMemory(512)).toBe('512 MB');
  });

  it('linhas do Warden traduzidas e as do jogo como saíram', () => {
    expect(consoleText(wardenLine('console.travou', { codigo: '255', duracaoMs: '48000' }))).toBe(
      'O jogo fechou com o código 255 depois de 48 s.',
    );
    expect(consoleText(gameLine('Ação, coração, pé'))).toBe('Ação, coração, pé');
    // Chave desconhecida (de uma versão mais nova do backend) aparece como veio.
    expect(consoleText(wardenLine('console.outra', {}))).toBe('console.outra');
    const text = consoleLineAsText(
      gameLine('olá', { atMs: null, time: '12:00:00', level: 'warn' }),
    );
    expect(text).toBe('12:00:00 [WARN] [minecraft/Minecraft] olá');
  });

  it('textos das etapas do backend', () => {
    expect(stageText('launcher.download')).toBe('Baixando os arquivos do jogo');
    expect(stageText('test.stage.syncPack')).toBe('Copiando o pack para o teste');
    expect(stageText('teste:etapa.abrir')).toBe('Abrindo o jogo');
    expect(stageText('java:etapa.download')).toBe('Baixando o Java');
    expect(stageText('nao.existe')).toBeNull();
  });
});
