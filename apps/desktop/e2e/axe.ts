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
 * ficam como estão. Um aviso também pode aparecer ou começar a sair **durante** a medição (o fim
 * de uma tarefa chega do backend a qualquer momento), e no WebKitGTK ele às vezes escapava da
 * checagem por eventos (`animationstart`) e por `getAnimations()` feita depois do `axe.run`. Então,
 * enquanto o axe roda, um `MutationObserver` olha cada elemento que entrou ou mudou de estado
 * (`class`, `style`, `hidden`, `open`, `data-state`): se ele, algo dentro dele ou em volta dele
 * tem animação ou transição finita, a medição é descartada e refeita depois de tudo parar. São no
 * máximo `MAX_RUNS` medições dentro do prazo; se a tela nunca para, o teste falha dizendo isso.
 * Nenhuma regra é desligada e nenhum elemento é excluído: o que está errado na tela parada
 * continua reprovando (`axe.e2e.ts` prova os dois lados).
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { browser } from '@wdio/globals';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

/** Quanto esperar, no total, as animações terminarem. */
const SETTLE_TIMEOUT_MS = 10_000;

/** Quantas vezes, no máximo, medir de novo porque algo se mexeu durante a medição. */
const MAX_RUNS = 5;

export interface AxeViolation {
  id: string;
  impact: string | null;
  help: string;
  nodes: { target: string[] }[];
}

/** As violações sérias e críticas da tela, medida sem animação em andamento. */
export async function seriousViolations(): Promise<AxeViolation[]> {
  await browser.execute(AXE_SOURCE);
  const violations = await browser.execute(
    async (timeoutMs: number, maxRuns: number) => {
      const { axe } = window as unknown as {
        axe: { run: (options: object) => Promise<{ violations: AxeViolation[] }> };
      };
      const deadline = performance.now() + timeoutMs;
      const finite = (animation: Animation) =>
        animation.effect?.getComputedTiming().endTime !== Infinity;
      const running = () =>
        document
          .getAnimations()
          .filter((animation) => animation.playState === 'running' && finite(animation));
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
      const seconds = (list: string) =>
        list.split(',').some((value) => Number.parseFloat(value) > 0);
      // O elemento se mexe: tem animação (CSS ou Web Animations) ou transição finita.
      const moves = (element: Element): boolean => {
        if (element.getAnimations().some(finite)) return true;
        const style = getComputedStyle(element);
        const animated =
          style.animationName !== '' &&
          style.animationName.split(',').some((name) => name.trim() !== 'none') &&
          !style.animationIterationCount.includes('infinite') &&
          seconds(style.animationDuration);
        return animated || seconds(style.transitionDuration);
      };
      // O que entrou ou mudou de estado: ele, o que está dentro e o que está em volta.
      const unstable = (element: Element): boolean => {
        if (!element.isConnected) return true;
        if (element.getAnimations({ subtree: true }).some(finite)) return true;
        for (let node: Element | null = element; node; node = node.parentElement) {
          if (moves(node)) return true;
        }
        return Array.from(element.querySelectorAll('*')).some(moves);
      };
      const result = (id: string, help: string): AxeViolation[] => [
        { id, impact: 'critical', help, nodes: [] },
      ];
      try {
        for (let run = 1; run <= maxRuns; run += 1) {
          if (!(await settle())) {
            return result(
              'animacao-sem-fim',
              `as animações não terminaram em ${String(timeoutMs / 1000)} s; o axe não mediu`,
            );
          }
          const changed = { any: false };
          const mark = () => {
            changed.any = true;
          };
          const check = (records: MutationRecord[]) => {
            for (const record of records) {
              if (changed.any) return;
              if (record.type === 'childList') {
                for (const node of record.addedNodes) {
                  if (node instanceof Element && unstable(node)) mark();
                }
              } else if (record.target instanceof Element && unstable(record.target)) {
                mark();
              }
            }
          };
          const observer = new MutationObserver(check);
          observer.observe(document.documentElement, {
            subtree: true,
            childList: true,
            attributes: true,
            attributeFilter: ['class', 'style', 'hidden', 'open', 'data-state'],
          });
          const before = new Set(document.getAnimations());
          document.addEventListener('animationstart', mark, true);
          document.addEventListener('transitionrun', mark, true);
          try {
            const { violations } = await axe.run({ resultTypes: ['violations'] });
            check(observer.takeRecords());
            const appeared = document
              .getAnimations()
              .some((animation) => !before.has(animation) && finite(animation));
            if (!changed.any && !appeared && running().length === 0) return violations;
          } finally {
            observer.disconnect();
            document.removeEventListener('animationstart', mark, true);
            document.removeEventListener('transitionrun', mark, true);
          }
        }
        return result(
          'tela-sem-parar',
          `a tela se mexeu durante todas as ${String(maxRuns)} medições; o axe não mediu`,
        );
      } catch {
        return result('axe-falhou', 'o axe não rodou');
      }
    },
    SETTLE_TIMEOUT_MS,
    MAX_RUNS,
  );
  return violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
}

/** As mesmas violações, como `regra: ajuda`, para comparar com `[]` e ler no erro. */
export async function seriousViolationSummaries(): Promise<string[]> {
  return (await seriousViolations()).map((violation) => `${violation.id}: ${violation.help}`);
}
