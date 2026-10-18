/**
 * Etapas do indicador da tela do teste (SPEC T13 "Fluxo ao clicar em Testar"). **Registro
 * acréscimo-apenas** (ROADMAP §1): a D-03 acrescenta "Verificar o pack" (antes de Preparar) e
 * "Verificação final" (antes de Abrir), com os seus `stage` (a pasta `verification/` é dela).
 * Até lá, esses passos não aparecem.
 *
 * Cada etapa diz quais etapas do backend (`OperationEvent.stage`) pertencem a ela.
 */
import i18n from '../../i18n';
import type { TestRequest } from '../../lib/ipc/bindings';

export interface TestStep {
  id: string;
  /** Posição (crescente): 100 verificar, 200 preparar, 300 copiar, 350 preparar o servidor
   * (L-09), 400 verificação final, 500 abrir. */
  order: number;
  /** Chave do texto com o namespace (`teste:etapas.preparar`). */
  labelKey: string;
  /** Se a etapa do backend pertence a este passo. */
  matches: (stage: string) => boolean;
  /** Só aparece nos testes em que vale (sem: em todos). */
  when?: (request: TestRequest) => boolean;
}

/** Registro acréscimo-apenas: uma linha por etapa. */
export const testSteps: readonly TestStep[] = [
  {
    id: 'check',
    order: 100,
    labelKey: 'diagnostico:etapas.verificar',
    matches: (stage) => stage === 'test.check',
  },
  {
    id: 'prepare',
    order: 200,
    labelKey: 'teste:etapas.preparar',
    matches: (stage) =>
      stage.startsWith('java') || stage.startsWith('launcher') || stage === 'test.java',
  },
  {
    id: 'sync',
    order: 300,
    labelKey: 'teste:etapas.copiar',
    // "Como o jogador recebe" (L-07): exportar e instalar pelo link ocupam este passo.
    matches: (stage) => stage === 'syncPack' || stage.startsWith('test.player.'),
  },
  {
    id: 'final-check',
    order: 400,
    labelKey: 'diagnostico:etapas.final',
    matches: (stage) => stage === 'test.finalCheck',
  },
  {
    id: 'launch',
    order: 500,
    labelKey: 'teste:etapas.abrir',
    matches: (stage) => stage === 'test.launch',
  },
  {
    // L-09: "Testar como servidor" instala o servidor e copia o pack só com os itens dele.
    id: 'server',
    order: 350,
    labelKey: 'servidor:etapas.preparar',
    matches: (stage) => stage === 'test.server',
    when: (request) => request.mode === 'server',
  },
];

/** Etapas que não existem em "Só o servidor" (o jogo não abre; L-09). */
const SERVER_ONLY_SKIPS = new Set(['sync', 'final-check']);

/**
 * As etapas de um teste, na ordem: as que valem para ele e, em "Testar como servidor", "Abrir
 * o jogo" vira "Abrir o servidor e o jogo" (ou "Abrir o servidor", em "Só o servidor").
 */
export function stepsFor(request: TestRequest, steps: readonly TestStep[] = testSteps): TestStep[] {
  const serverOnly = request.mode === 'server' && request.serverOnly === true;
  return orderedSteps(steps)
    .filter((step) => step.when?.(request) ?? true)
    .filter((step) => !(serverOnly && SERVER_ONLY_SKIPS.has(step.id)))
    .map((step) =>
      request.mode === 'server' && step.id === 'launch'
        ? {
            ...step,
            labelKey: serverOnly ? 'servidor:etapas.abrirSo' : 'servidor:etapas.abrir',
          }
        : step,
    );
}

/** As etapas na ordem. */
export function orderedSteps(steps: readonly TestStep[] = testSteps): TestStep[] {
  return [...steps].sort((a, b) => a.order - b.order);
}

/** O texto de uma etapa. */
export function stepLabel(step: TestStep): string {
  return (i18n.t as (key: string) => string)(step.labelKey);
}

/**
 * O passo atual: o mais adiantado entre as etapas do backend já vistas (o indicador nunca
 * volta). Com o jogo aberto, todos ficam concluídos (`steps.length`).
 */
export function currentStep(
  stagesSeen: readonly string[],
  running: boolean,
  steps: readonly TestStep[] = stepsFor({ mode: 'normal' }),
): number {
  if (running) {
    return steps.length;
  }
  let current = 0;
  for (const stage of stagesSeen) {
    const index = steps.findIndex((step) => step.matches(stage));
    if (index > current) current = index;
  }
  return current;
}
