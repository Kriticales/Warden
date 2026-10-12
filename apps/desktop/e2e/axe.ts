/**
 * Ajuda comum dos E2E: rodar o axe-core dentro do WebView e devolver as violações sérias e
 * críticas (QUALITY §4.1).
 *
 * O axe mede o contraste com a opacidade do momento. Um aviso (toast) entrando (`toast-in`) ou
 * saindo (`fade-out`) passa por opacidades intermediárias durante 200 ms, e o texto dele, nesse
 * meio-tempo, reprova em `color-contrast` (com opacidade 0,3 o contraste cai para ~2,5:1). Os
 * avisos duram 6 s e as etapas dos testes rodam em poucos segundos, então às vezes a medição caía
 * bem no meio de uma animação e o teste falhava sem nada errado na tela pronta.
 *
 * Por isso, antes de medir, espera todas as animações e transições finitas terminarem
 * (`document.getAnimations()`); as infinitas, como o pulso do botão Testar, não terminam nunca e
 * ficam como estão. Se alguma começar enquanto o axe roda, mede de novo. Nenhuma regra é
 * desligada e nenhum elemento é excluído.
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { browser } from '@wdio/globals';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

/** Quanto esperar, no total, as animações terminarem. */
const SETTLE_TIMEOUT_MS = 10_000;

export interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
  nodes: { target: string[] }[];
}

/** As violações sérias e críticas da tela, medida sem animação em andamento. */
export async function seriousViolations(): Promise<AxeViolation[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(async (timeoutMs: number) => {
    const { axe } = window as unknown as {
      axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
    };
    const deadline = performance.now() + timeoutMs;
    const running = () =>
      document
        .getAnimations()
        .filter(
          (animation) =>
            animation.playState === 'running' &&
            animation.effect?.getComputedTiming().endTime !== Infinity,
        );
    // Espera as animações finitas em andamento; as que começarem enquanto isso também.
    const settle = async (): Promise<boolean> => {
      for (;;) {
        const pending = running();
        if (pending.length === 0) return true;
        const left = deadline - performance.now();
        if (left <= 0) return false;
        await Promise.race([
          Promise.allSettled(pending.map((animation) => animation.finished)),
          new Promise((done) => setTimeout(done, left)),
        ]);
      }
    };
    const unsettled: AxeViolation = {
      id: 'animacao-sem-fim',
      impact: 'critical',
      help: `as animações não terminaram em ${String(timeoutMs / 1000)} s; o axe não mediu`,
      nodes: [],
    };
    try {
      while (await settle()) {
        const started = { any: false };
        const mark = () => {
          started.any = true;
        };
        document.addEventListener('animationstart', mark, true);
        document.addEventListener('transitionrun', mark, true);
        try {
          const result = await axe.run({ resultTypes: ['violations'] });
          if (!started.any && running().length === 0) return result.violations;
        } finally {
          document.removeEventListener('animationstart', mark, true);
          document.removeEventListener('transitionrun', mark, true);
        }
      }
      return [unsettled];
    } catch {
      const failed: AxeViolation = {
        id: 'axe-falhou',
        impact: 'critical',
        help: 'o axe não rodou',
        nodes: [],
      };
      return [failed];
    }
  }, SETTLE_TIMEOUT_MS);
  return violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
}

/** As mesmas violações, como `regra: ajuda`, para comparar com `[]` e ler no erro. */
export async function seriousViolationSummaries(): Promise<string[]> {
  return (await seriousViolations()).map((violation) => `${violation.id}: ${violation.help}`);
}
