/**
 * Regra do diálogo de dependências (SPEC T09; testada em `model.test.ts`): o que entra no pack
 * a partir do plano (`add_plan`) e das escolhas do usuário, e o pedido de gravação
 * (`add_apply`).
 *
 * - **Escolhido:** entra se continuar marcado. Se já está no pack por outra fonte, só entra
 *   quando o usuário escolhe substituir (manter os dois faria o jogo travar).
 * - **Obrigatória:** entra quando algum item que a pede entra, salvo se o usuário desmarcar.
 * - **Opcional:** só entra se o usuário marcar e algum item para quem ela é opcional entrar.
 *
 * O fecho é transitivo: a obrigatória de uma opcional só entra se a opcional entrar; a de uma
 * obrigatória desmarcada também sai (a não ser que outro item que entra a peça).
 */
import type { AddApplyRequest, AddPlan, PlanNode } from '../../../lib/ipc/bindings';

/** O que o usuário mudou no diálogo. */
export interface DependencyChoices {
  /** Itens escolhidos ou obrigatórias desmarcados. */
  unchecked: ReadonlySet<string>;
  /** Opcionais marcadas. */
  optional: ReadonlySet<string>;
  /** Duplicados entre fontes que o usuário mandou substituir. */
  replace: ReadonlySet<string>;
}

export const NO_CHOICES: DependencyChoices = {
  unchecked: new Set(),
  optional: new Set(),
  replace: new Set(),
};

/** As chaves dos nós que vão entrar no pack. */
export function includedKeys(plan: AddPlan, choices: DependencyChoices): Set<string> {
  const duplicates = new Set(plan.duplicates.map((duplicate) => duplicate.key));
  const included = new Set<string>();
  for (const node of plan.nodes) {
    if (node.role !== 'chosen' || choices.unchecked.has(node.key)) continue;
    if (duplicates.has(node.key) && !choices.replace.has(node.key)) continue;
    included.add(node.key);
  }
  // Fecho: cada obrigatória entra quando alguém que a pede entrou; cada opcional marcada, quando
  // algum item para quem ela é opcional entrou (desmarcado o mod que a pedia, ela sai, como a
  // caixa desabilitada mostra).
  let changed = true;
  while (changed) {
    changed = false;
    for (const node of plan.nodes) {
      if (included.has(node.key)) continue;
      if (node.role === 'optional') {
        if (choices.optional.has(node.key) && node.optionalFor.some((key) => included.has(key))) {
          included.add(node.key);
          changed = true;
        }
        continue;
      }
      if (node.role !== 'required') continue;
      if (choices.unchecked.has(node.key)) continue;
      if (node.requiredBy.some((key) => included.has(key))) {
        included.add(node.key);
        changed = true;
      }
    }
  }
  return included;
}

/**
 * O pedido de gravação: os nós que entram, na ordem do plano, com o lado que o plano mostrou
 * (o da CurseForge pode vir do Modrinth, pelo mesmo arquivo) e `replaces` dos duplicados.
 */
export function applyRequest(
  plan: AddPlan,
  included: ReadonlySet<string>,
  choices: DependencyChoices,
): AddApplyRequest {
  const replaced = new Map(
    plan.duplicates
      .filter((duplicate) => choices.replace.has(duplicate.key))
      .map((duplicate) => [duplicate.key, duplicate.existingPath]),
  );
  return {
    items: plan.nodes
      .filter((node) => included.has(node.key))
      .map((node) => ({
        source: node.source,
        projectId: node.projectId,
        versionId: node.versionId,
        side: node.side,
        replaces: replaced.get(node.key) ?? null,
      })),
  };
}

/**
 * O que o título e o botão contam, a mesma conta: os escolhidos que entram e as dependências
 * (obrigatórias e opcionais marcadas) que entram com eles.
 */
export function includedCounts(
  plan: AddPlan,
  included: ReadonlySet<string>,
): { chosen: PlanNode[]; dependencies: number } {
  const chosen = plan.nodes.filter((node) => node.role === 'chosen' && included.has(node.key));
  return { chosen, dependencies: included.size - chosen.length };
}

/** Só as chaves dos itens que entram (para "Pedida por X", sem quem foi desmarcado). */
export function onlyIncluded(keys: readonly string[], included: ReadonlySet<string>): string[] {
  return keys.filter((key) => included.has(key));
}

/** Os nomes dos itens do plano pelas chaves (para "Pedida por X e Y"). */
export function titlesOf(plan: AddPlan, keys: readonly string[]): string[] {
  const byKey = new Map<string, string>(plan.nodes.map((node) => [node.key, node.title]));
  for (const installed of plan.installed) byKey.set(installed.key, installed.title);
  return keys.map((key) => byKey.get(key) ?? key);
}

/** Os nós de um papel, na ordem do plano. */
export function nodesOf(plan: AddPlan, role: PlanNode['role']): PlanNode[] {
  return plan.nodes.filter((node) => node.role === role);
}

/** Lista legível em pt-BR: "A", "A e B", "A, B e C". */
export function joinNames(names: readonly string[]): string {
  return new Intl.ListFormat('pt-BR', { style: 'long', type: 'conjunction' }).format(names);
}
