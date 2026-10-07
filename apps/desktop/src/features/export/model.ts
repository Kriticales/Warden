/**
 * Modelo da árvore "O que vai no pack" (SPEC T19, pré-visualização): os arquivos de controle,
 * um grupo por pasta de primeiro nível (com contagem, referências e tamanho) e os arquivos soltos
 * na raiz; e a lista plana dos alertas, para o painel "pedem atenção".
 */
import type { ExportAlert, ExportPreview, PreviewFile } from '../../lib/ipc/bindings';

/** Manifesto e índice: vão sempre e não podem ser excluídos. */
export const CONTROL_FILES = ['pack.toml', 'index.toml'] as const;

export function isControl(path: string): boolean {
  return (CONTROL_FILES as readonly string[]).includes(path);
}

/** Uma pasta de primeiro nível, ou a raiz (`folder: null`) com os arquivos soltos. */
export interface TreeGroup {
  folder: string | null;
  files: PreviewFile[];
  /** Quantos arquivos vão (inclusive as referências). */
  count: number;
  /** Quantos são referências `.pw.toml` (o jar não vai). */
  references: number;
  bytes: number;
}

export interface PreviewTree {
  control: PreviewFile[];
  groups: TreeGroup[];
}

function topFolder(path: string): string | null {
  const slash = path.indexOf('/');
  return slash < 0 ? null : path.slice(0, slash);
}

/**
 * Agrupa a prévia. A contagem e o tamanho de cada pasta vêm de `preview.folders` (os totais do
 * backend); o que não estiver lá é somado dos arquivos. Os arquivos soltos ficam por último.
 */
export function buildTree(preview: ExportPreview): PreviewTree {
  const control = CONTROL_FILES.flatMap((path) => preview.files.filter((f) => f.path === path));
  const byFolder = new Map<string | null, PreviewFile[]>();
  for (const file of preview.files) {
    if (isControl(file.path)) continue;
    const folder = topFolder(file.path);
    const list = byFolder.get(folder) ?? [];
    list.push(file);
    byFolder.set(folder, list);
  }
  const groups = [...byFolder.entries()].map(([folder, files]): TreeGroup => {
    const totals = folder === null ? undefined : preview.folders.find((f) => f.path === folder);
    return {
      folder,
      files,
      count: totals?.files ?? files.length,
      references: files.filter((file) => file.reference).length,
      bytes: totals?.bytes ?? files.reduce((sum, file) => sum + file.bytes, 0),
    };
  });
  groups.sort((a, b) => {
    if (a.folder === null) return 1;
    if (b.folder === null) return -1;
    return a.folder.localeCompare(b.folder);
  });
  return { control, groups };
}

/** Um alerta de um arquivo. */
export interface FileAlert {
  path: string;
  alert: ExportAlert;
}

export function collectAlerts(preview: ExportPreview): FileAlert[] {
  return preview.files.flatMap((file) => file.alerts.map((alert) => ({ path: file.path, alert })));
}

/** A linha que `export_exclude` acrescenta ao `.packwizignore` (mostrada antes de confirmar). */
export function ignoreRule(path: string, folder: boolean): string {
  return `/${path}${folder ? '/' : ''}`;
}

/** Se "Excluir do pack" é possível: nunca o controle, e colchetes pedem a regra à mão. */
export function canExclude(path: string): boolean {
  return !isControl(path) && !/[[\]]/.test(path);
}
