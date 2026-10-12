/** Dados de teste das atualizações (P1-12): itens do pack, relatório, revisão. */
import type {
  InventoryItem,
  ModUpdateReport,
  PlanItem,
  UpdateItem,
  UpdatePlan,
} from '../../lib/ipc/bindings';
import { makeItem } from '../pack-editor/editor.fixtures';

export const ALFA = makeItem({
  key: 'modrinth:projA',
  path: 'mods/alfa.pw.toml',
  name: 'Alfa',
  version: '1.0',
  projectId: 'projA',
  sourceVersionId: 'oldA',
});

export const BETA = makeItem({
  key: 'modrinth:projB',
  path: 'mods/beta.pw.toml',
  name: 'Beta',
  version: '2.0',
  projectId: 'projB',
  sourceVersionId: 'oldB',
});

export const GAMA = makeItem({
  key: 'curseforge:300',
  path: 'mods/gama.pw.toml',
  name: 'Gama',
  version: 'gama-1',
  source: 'curseforge',
  projectId: '300',
  sourceVersionId: '500',
});

export const FIXADO = makeItem({
  key: 'modrinth:projF',
  path: 'mods/fixado.pw.toml',
  name: 'Fixado',
  version: '3.0',
  pinned: true,
  projectId: 'projF',
  sourceVersionId: 'oldF',
});

export function updateItem(item: InventoryItem, overrides: Partial<UpdateItem> = {}): UpdateItem {
  return {
    key: item.key,
    path: item.path,
    name: item.name,
    source: item.source,
    status: 'upToDate',
    currentId: item.sourceVersionId,
    current: item.version,
    newVersion: null,
    reason: null,
    detail: null,
    ...overrides,
  };
}

export function available(item: InventoryItem, number: string, id = `new-${item.name}`) {
  return updateItem(item, {
    status: 'available',
    newVersion: { id, number, channel: 'release', published: '2026-10-01T10:00:00Z' },
  });
}

export function makeReport(items: UpdateItem[]): ModUpdateReport {
  return {
    checkedAt: '2026-10-06T17:01:00Z',
    items,
    available: items.filter((item) => item.status === 'available').length,
  };
}

export function planItem(update: UpdateItem, overrides: Partial<PlanItem> = {}): PlanItem {
  if (!update.newVersion) throw new Error('item sem versão nova');
  return {
    key: update.key,
    path: update.path,
    name: update.name,
    source: update.source,
    current: update.current,
    newVersion: update.newVersion,
    changelog: [
      {
        version: update.newVersion.number,
        channel: 'release',
        published: '2026-10-01T10:00:00Z',
        text: `Notas da ${update.newVersion.number}`,
        format: 'markdown',
      },
    ],
    manualDownload: false,
    ...overrides,
  };
}

export function makePlan(items: PlanItem[], overrides: Partial<UpdatePlan> = {}): UpdatePlan {
  return { items, newDependencies: [], conflicts: [], ...overrides };
}
