/**
 * O que a tela de "O que mudou durante o teste" guarda enquanto a pessoa escolhe: os arquivos
 * marcados, as chaves do `options.txt` marcadas e a escolha de cada conflito. Começa com a
 * pré-seleção da SPEC T15 (configs do pack e chaves que já existiam marcadas; o resto
 * desmarcado; conflito em "Juntar as duas" quando dá, senão em "Manter a do pack").
 */
import type {
  CaptureApplyRequest,
  CaptureFileView,
  CaptureGroup,
  CaptureResolution,
  CaptureView,
} from '../../lib/ipc/bindings';

export interface Selection {
  /** Caminho → marcado (configs do pack sem conflito, configs de mundo, arquivos criados). */
  files: Readonly<Record<string, boolean>>;
  /** Chave do `options.txt` → marcada. */
  options: Readonly<Record<string, boolean>>;
  /** Caminho → escolha do conflito. */
  conflicts: Readonly<Record<string, CaptureResolution>>;
}

/** Os arquivos da captura separados como a tela os mostra. */
export interface Groups {
  conflicts: CaptureFileView[];
  byGroup: Record<CaptureGroup, CaptureFileView[]>;
  options: CaptureFileView | null;
}

/** Grupos com caixa de marcar (os outros só informam ou oferecem adicionar pelo Adicionar). */
const SELECTABLE: readonly CaptureGroup[] = ['packConfig', 'worldConfig', 'generated'];

export function isConflict(view: CaptureFileView): boolean {
  return view.conflict !== null && view.file.group === 'packConfig';
}

export function groupsOf(view: CaptureView): Groups {
  const byGroup: Record<CaptureGroup, CaptureFileView[]> = {
    packConfig: [],
    options: [],
    worldConfig: [],
    generated: [],
    mod: [],
    resource: [],
    datapack: [],
  };
  const conflicts: CaptureFileView[] = [];
  for (const file of view.files) {
    if (isConflict(file)) conflicts.push(file);
    else byGroup[file.file.group].push(file);
  }
  return { conflicts, byGroup, options: byGroup.options[0] ?? null };
}

/** Chaves do `options.txt` que também mudaram no pack durante o teste. */
export function packChangedKeys(options: CaptureFileView | null): ReadonlyMap<string, string> {
  return new Map(
    (options?.conflict?.packValues ?? []).map((value) => [value.key, value.after ?? ''] as const),
  );
}

export function initialSelection(view: CaptureView): Selection {
  const groups = groupsOf(view);
  const files: Record<string, boolean> = {};
  for (const group of SELECTABLE) {
    for (const file of groups.byGroup[group]) files[file.file.path] = file.file.selected;
  }
  const changedInPack = packChangedKeys(groups.options);
  const options: Record<string, boolean> = {};
  for (const option of groups.options?.file.options ?? []) {
    options[option.key] = option.selected && !changedInPack.has(option.key);
  }
  const conflicts: Record<string, CaptureResolution> = {};
  for (const file of groups.conflicts) {
    conflicts[file.file.path] = file.conflict?.mergeable ? { kind: 'merge' } : { kind: 'keepPack' };
  }
  return { files, options, conflicts };
}

/** Quantas mudanças vão para o pack (cada arquivo e cada chave contam uma). */
export function selectedCount(selection: Selection): number {
  const files = Object.values(selection.files).filter(Boolean).length;
  const options = Object.values(selection.options).filter(Boolean).length;
  const conflicts = Object.values(selection.conflicts).filter(
    (choice) => choice.kind !== 'keepPack',
  ).length;
  return files + options + conflicts;
}

/** O pedido de "Trazer selecionados para o pack". */
export function buildRequest(view: CaptureView, selection: Selection): CaptureApplyRequest {
  const groups = groupsOf(view);
  const files: CaptureApplyRequest['files'] = [];
  for (const file of view.files) {
    const path = file.file.path;
    const conflict = isConflict(file) ? selection.conflicts[path] : undefined;
    if (conflict) {
      if (conflict.kind !== 'keepPack') {
        files.push({ path, resolution: conflict, expectedPackSha256: file.packSha256 });
      }
    } else if (selection.files[path] === true) {
      files.push({ path, resolution: { kind: 'take' }, expectedPackSha256: file.packSha256 });
    }
  }
  const optionKeys = (groups.options?.file.options ?? [])
    .map((option) => option.key)
    .filter((key) => selection.options[key] === true);
  return {
    sessionId: view.sessionId,
    files,
    optionKeys,
    expectedOptionsSha256: groups.options?.packSha256 ?? null,
  };
}
