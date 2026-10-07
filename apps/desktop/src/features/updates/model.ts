/**
 * Lógica das atualizações, sem React: o estado efetivo de cada linha da lista a partir do
 * relatório (SPEC T10). O relatório é do momento da verificação; se o item mudou depois
 * (outra versão instalada), o resultado antigo não vale e a linha volta a "Não verificado".
 */
import type {
  InventoryItem,
  UpdateItem,
  UpdateReason,
  ModUpdateReport,
  UpdateStatus,
} from '../../lib/ipc/bindings';

/** O resultado do relatório para um item, ou `null` se não vale mais ou não existe. */
export function reportItem(
  report: ModUpdateReport | null | undefined,
  item: InventoryItem,
): UpdateItem | null {
  const found = report?.items.find((entry) => entry.path === item.path);
  if (!found) return null;
  return (found.currentId ?? null) === (item.sourceVersionId ?? null) ? found : null;
}

/** O estado que a linha mostra. */
export function effectiveStatus(
  report: ModUpdateReport | null | undefined,
  item: InventoryItem,
): UpdateStatus {
  if (item.state !== 'ok') return 'notApplicable';
  if (item.pinned) return 'pinned';
  if (item.source === 'local' || item.source === 'url') return 'notApplicable';
  return reportItem(report, item)?.status ?? 'notChecked';
}

/** Itens com atualização disponível (os fixados nunca entram). */
export function updatableItems(
  report: ModUpdateReport | null | undefined,
  items: readonly InventoryItem[],
): InventoryItem[] {
  return items.filter((item) => effectiveStatus(report, item) === 'available');
}

export interface UpdateCounts {
  available: number;
  failed: number;
  /** Itens da CurseForge que ficaram sem verificar por falta ou recusa da chave. */
  keyProblem: number;
}

export function countUpdates(
  report: ModUpdateReport | null | undefined,
  items: readonly InventoryItem[],
): UpdateCounts {
  const counts: UpdateCounts = { available: 0, failed: 0, keyProblem: 0 };
  for (const item of items) {
    const status = effectiveStatus(report, item);
    if (status === 'available') counts.available += 1;
    if (status === 'failed' && !hasKeyProblem(reportItem(report, item)?.reason)) {
      counts.failed += 1;
    }
    if (hasKeyProblem(reportItem(report, item)?.reason)) counts.keyProblem += 1;
  }
  return counts;
}

export function hasKeyProblem(reason: UpdateReason | null | undefined): boolean {
  return reason === 'keyMissing' || reason === 'keyInvalid';
}
