/**
 * Etapas do indicador da tela do teste (SPEC T13 "Fluxo ao clicar em Testar"). **Registro
 * acréscimo-apenas** (ROADMAP §1): a D-03 acrescenta "Verificar o pack" (antes de Preparar) e
 * "Verificação final" (antes de Abrir), com os seus `stage` (a pasta `verification/` é dela).
 * Até lá, esses passos não aparecem.
 *
 * Cada etapa diz quais etapas do backend (`OperationEvent.stage`) pertencem a ela.
 */
import i18n from '../../i18n';

export interface TestStep {
  id: string;
  /** Posição (crescente): 100 verificar, 200 preparar, 300 copiar, 400 verificação final,
   * 500 abrir. */
  order: number;
  /** Chave do texto com o namespace (`teste:etapas.preparar`). */
  labelKey: string;
  /** Se a etapa do backend pertence a este passo. */
  matches: (stage: string) => boolean;
}

/** Registro acréscimo-apenas: uma linha por etapa. */
export const testSteps: readonly TestStep[] = [
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
    matches: (stage) => stage === 'syncPack',
  },
  {
    id: 'launch',
    order: 500,
    labelKey: 'teste:etapas.abrir',
    matches: (stage) => stage === 'test.launch',
  },
];

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
  steps: readonly TestStep[] = orderedSteps(),
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
