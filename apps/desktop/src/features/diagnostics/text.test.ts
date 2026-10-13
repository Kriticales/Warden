/**
 * Frases dos achados (D-03): escolha da frase pelo código da regra ou pela chave do padrão de
 * log, a frase genérica para o que o catálogo não conhece, as correções e a contagem.
 */
import { describe, expect, it } from 'vitest';

import type { ReportFinding, SuggestedFix } from '../../lib/ipc/bindings';

import { problemsByPath } from './components/ProblemMark';
import {
  CRASH_CAUSE,
  MISSING_DEP,
  PRERELEASE,
  WRONG_SIDE,
  makeEntry,
} from './diagnostics.fixtures';
import {
  countBySeverity,
  findingText,
  findingTitle,
  fixAvailable,
  fixLabel,
  formatAgo,
  phraseKey,
} from './text';

function firstFix(entry: ReportFinding): SuggestedFix {
  const [fix] = entry.finding.fixes;
  if (!fix) throw new Error('achado sem correção');
  return fix;
}

describe('frases dos achados', () => {
  it('pré-teste pelo código; pós-crash pela chave do padrão', () => {
    expect(phraseKey(MISSING_DEP)).toBe('regras.E_MISSING_DEP');
    expect(findingTitle(MISSING_DEP)).toBe('Meu Mod precisa de Sodium, que não está no pack');
    expect(phraseKey(CRASH_CAUSE)).toBe('achados.dependenciaFaltando');
    expect(findingText(CRASH_CAUSE)).toBe(
      'O loader parou de carregar porque falta a dependência balm ([7.3.0,)). Adicione a dependência.',
    );
  });

  it('Java do pack sem item tem frase própria; regra desconhecida usa a genérica', () => {
    const java = makeEntry({ rule: 'E_JAVA' });
    expect(findingTitle(java)).toBe('O Java do teste não serve para esta versão do Minecraft');
    const unknown = makeEntry({ rule: 'W_REGRA_NOVA', titleKey: 'diagnostics.pretest.x' });
    expect(phraseKey(unknown)).toBeNull();
    expect(findingTitle(unknown)).toBe('Problema encontrado (W_REGRA_NOVA)');
    // Parâmetro que o log não trouxe aparece como "?", nunca como "{{x}}".
    const partial = makeEntry({
      rule: 'E_CRASH_REPORT',
      titleKey: 'diagnostico.achados.crashReport',
      params: {},
    });
    expect(findingTitle(partial)).toBe('O jogo travou: ?');
  });

  it('textos das correções e quando valem', () => {
    expect(fixLabel(MISSING_DEP, firstFix(MISSING_DEP))).toBe('Adicionar Sodium');
    expect(fixLabel(WRONG_SIDE, firstFix(WRONG_SIDE))).toBe('Mudar lado para Só cliente');
    const item = { kind: 'metafile' as const, path: 'mods/sodium.pw.toml' };
    expect(fixLabel(WRONG_SIDE, { kind: 'removeItem', item })).toBe('Remover Sodium');
    expect(fixLabel(WRONG_SIDE, { kind: 'updateItem', item })).toBe('Atualizar Sodium');
    expect(fixLabel(WRONG_SIDE, { kind: 'changeJava', major: 21 })).toBe('Usar o Java 21 no teste');
    expect(fixLabel(WRONG_SIDE, { kind: 'changeMemory' })).toBe('Aumentar a memória do teste');
    expect(fixLabel(WRONG_SIDE, { kind: 'removeJvmArgument', argument: '-XX:+UseZGC' })).toBe(
      'Tirar -XX:+UseZGC dos argumentos',
    );
    // Remover um mod que não está no pack não tem botão.
    expect(
      fixAvailable(CRASH_CAUSE, { kind: 'removeItem', item: { kind: 'modId', id: 'balm' } }),
    ).toBe(false);
    expect(fixAvailable(WRONG_SIDE, firstFix(WRONG_SIDE))).toBe(true);
  });

  it('contagem sem os ignorados e o pior problema de cada item', () => {
    const ignored = { ...WRONG_SIDE, ignored: true };
    expect(countBySeverity([MISSING_DEP, ignored, PRERELEASE])).toEqual({
      error: 1,
      warning: 0,
      info: 1,
    });
    const byPath = problemsByPath([MISSING_DEP, WRONG_SIDE, PRERELEASE]);
    expect(byPath.get('mods/meu-mod.jar')).toEqual({ severity: 'error', count: 1 });
    expect(byPath.get('mods/sodium.pw.toml')).toEqual({ severity: 'warning', count: 1 });
    expect(problemsByPath([ignored]).size).toBe(0);
  });

  it('"há quanto tempo"', () => {
    const now = Date.UTC(2026, 9, 13, 12, 0, 0);
    expect(formatAgo(now - 10_000, now)).toBe('agora há pouco');
    expect(formatAgo(now - 3 * 60_000, now)).toBe('há 3 minutos');
    expect(formatAgo(now - 2 * 3_600_000, now)).toBe('há 2 horas');
    expect(formatAgo(now - 26 * 3_600_000, now)).toBe('ontem');
  });
});
