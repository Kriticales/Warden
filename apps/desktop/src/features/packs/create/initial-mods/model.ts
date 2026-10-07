/**
 * Regras da etapa "Mods iniciais" (SPEC T03; ADR-0033; decisão D16), sem React: o que vem
 * marcado, o que não pode ser marcado e o pedido que o assistente manda depois do "Pack
 * criado".
 */
import type {
  InitialModsRequest,
  InitialOffer,
  Kit,
  KitChoice,
  OfferItem,
} from '../../../../lib/ipc/bindings';

/** A escolha do jogador nesta etapa. */
export interface InitialChoice {
  /** Identificadores das ferramentas marcadas (`spark`, `crash-assistant`). */
  tools: string[];
  /** Kit de desempenho escolhido, com os mods que continuam marcados. */
  kit: KitChoice | null;
}

/** Nenhuma ferramenta e nenhum kit (quem não consulta, ou desmarcou tudo). */
export const NO_INITIAL: InitialChoice = { tools: [], kit: null };

/** Se o item pode ser marcado (a CurseForge desligada ou a falta de versão desabilitam). */
export function selectable(item: OfferItem): boolean {
  return item.unavailable === null;
}

/** O que vem marcado quando a oferta chega: as ferramentas marcadas nos dados e disponíveis. */
export function defaultChoice(offer: InitialOffer): InitialChoice {
  return {
    tools: offer.items.filter((item) => item.checked && selectable(item)).map((item) => item.id),
    kit: null,
  };
}

/** Liga ou desliga uma ferramenta. */
export function withTool(choice: InitialChoice, id: string, on: boolean): InitialChoice {
  const rest = choice.tools.filter((tool) => tool !== id);
  return { ...choice, tools: on ? [...rest, id] : rest };
}

/** Escolhe um kit (com os itens marcados por padrão) ou tira o kit (`null`). */
export function withKit(choice: InitialChoice, kit: Kit | null): InitialChoice {
  return {
    ...choice,
    kit:
      kit === null
        ? null
        : {
            id: kit.id,
            projects: kit.items.filter((item) => item.checked).map((item) => item.projectId),
          },
  };
}

/** Liga ou desliga um mod do kit escolhido. Sem kit escolhido, não muda nada. */
export function withKitProject(choice: InitialChoice, projectId: string): InitialChoice {
  if (choice.kit === null) return choice;
  const has = choice.kit.projects.includes(projectId);
  return {
    ...choice,
    kit: {
      ...choice.kit,
      projects: has
        ? choice.kit.projects.filter((project) => project !== projectId)
        : [...choice.kit.projects, projectId],
    },
  };
}

/** Se há algo a gravar depois de criar o pack. */
export function hasWork(choice: InitialChoice): boolean {
  return choice.tools.length > 0 || (choice.kit !== null && choice.kit.projects.length > 0);
}

/** O pedido de `initial_mods_apply`; `null` quando não há nada a gravar. */
export function toRequest(choice: InitialChoice): InitialModsRequest | null {
  if (!hasWork(choice)) return null;
  return {
    tools: choice.tools,
    kit: choice.kit !== null && choice.kit.projects.length > 0 ? choice.kit : null,
  };
}

/** Os nomes das ferramentas marcadas, na ordem da oferta (para o resumo). */
export function toolNames(offer: InitialOffer | undefined, choice: InitialChoice): string[] {
  if (offer === undefined) return [];
  return offer.items.filter((item) => choice.tools.includes(item.id)).map((item) => item.title);
}
