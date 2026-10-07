/**
 * A seleção dos mods de um kit (ADR-0033): quais itens vêm marcados e o que cada marca vira no
 * pedido. Sem React, para testar a regra sozinha.
 */
import type { AddChoice, Kit, KitChoice, KitItem } from '../../../lib/ipc/bindings';

/** IDs dos projetos que o kit traz marcados. */
export function defaultSelection(kit: Kit): Set<string> {
  return new Set(kit.items.filter((item) => item.checked).map((item) => item.projectId));
}

/** Liga ou desliga um item, sem mudar a seleção recebida. */
export function toggled(selection: ReadonlySet<string>, projectId: string): Set<string> {
  const next = new Set(selection);
  if (!next.delete(projectId)) next.add(projectId);
  return next;
}

/** Os itens do kit que continuam marcados, na ordem do kit. */
export function selectedItems(kit: Kit, selection: ReadonlySet<string>): KitItem[] {
  return kit.items.filter((item) => selection.has(item.projectId));
}

/** O que o assistente manda ao gravar: o kit e os projetos marcados. */
export function kitChoice(kit: Kit, selection: ReadonlySet<string>): KitChoice {
  return { id: kit.id, projects: selectedItems(kit, selection).map((item) => item.projectId) };
}

/** Os itens marcados como escolhas do plano (diálogo de dependências). */
export function addChoices(kit: Kit, selection: ReadonlySet<string>): AddChoice[] {
  return selectedItems(kit, selection).map((item) => ({
    source: item.source,
    projectId: item.projectId,
  }));
}
