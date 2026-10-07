/**
 * Modelo do editor de configs (SPEC T12): a árvore de arquivos, o filtro por nome, a linguagem
 * do realce e os avisos contextuais. Funções puras, sem IPC.
 */
import type { ConfigFile, ConfigOrigin } from '../../lib/ipc/bindings';

/** Um nó da árvore de arquivos. */
export type TreeNode = DirNode | FileNode;

export interface DirNode {
  kind: 'dir';
  /** Nome mostrado; pastas com um único filho pasta viram uma só ("saves/Mundo/serverconfig"). */
  name: string;
  /** Caminho completo, único (serve de chave). */
  path: string;
  children: TreeNode[];
  /** Arquivos dentro da pasta, em qualquer nível. */
  count: number;
}

export interface FileNode {
  kind: 'file';
  name: string;
  path: string;
  file: ConfigFile;
}

/** Ordem natural, sem diferenciar maiúsculas ("a2" antes de "a10"). */
export function compareNames(left: string, right: string): number {
  return left.localeCompare(right, 'pt-BR', { numeric: true, sensitivity: 'base' });
}

interface MutableDir {
  name: string;
  path: string;
  dirs: Map<string, MutableDir>;
  files: FileNode[];
}

function finish(dir: MutableDir): DirNode {
  const dirs = [...dir.dirs.values()].map(finish).sort((a, b) => compareNames(a.name, b.name));
  const files = [...dir.files].sort((a, b) => compareNames(a.name, b.name));
  let node: DirNode = {
    kind: 'dir',
    name: dir.name,
    path: dir.path,
    children: [...dirs, ...files],
    count: dirs.reduce((sum, child) => sum + child.count, 0) + files.length,
  };
  // Pasta com uma única subpasta e nenhum arquivo: junta os nomes.
  while (node.children.length === 1 && node.children[0]?.kind === 'dir') {
    const only: DirNode = node.children[0];
    node = { ...only, name: `${node.name}/${only.name}` };
  }
  return node;
}

/**
 * Monta a árvore a partir da lista plana: pastas primeiro, depois arquivos, em ordem natural.
 * Os arquivos soltos na raiz (`options.txt`) vêm depois das pastas.
 */
export function buildTree(files: readonly ConfigFile[]): TreeNode[] {
  const root: MutableDir = { name: '', path: '', dirs: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split('/');
    const name = parts.pop() ?? file.path;
    let dir = root;
    for (const part of parts) {
      const path = dir.path === '' ? part : `${dir.path}/${part}`;
      let next = dir.dirs.get(part);
      if (!next) {
        next = { name: part, path, dirs: new Map(), files: [] };
        dir.dirs.set(part, next);
      }
      dir = next;
    }
    dir.files.push({ kind: 'file', name, path: file.path, file });
  }
  return finish(root).children;
}

/** Texto sem acento e em minúsculas, para o filtro. */
export function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

/** Arquivos cujo caminho contém todas as palavras do filtro (sem acento nem maiúsculas). */
export function filterFiles(files: readonly ConfigFile[], query: string): ConfigFile[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    return [...files];
  }
  return files.filter((file) => {
    const path = fold(file.path);
    return words.every((word) => path.includes(word));
  });
}

/** Linguagem do realce de sintaxe. */
export type EditorLanguage =
  'toml' | 'json' | 'yaml' | 'properties' | 'javascript' | 'typescript' | 'plain';

/** A linguagem pelo nome do arquivo (SPEC T12: TOML, JSON, JSON5, YAML, `.properties`, `.cfg`, JS). */
export function languageOf(path: string): EditorLanguage {
  const name = (path.split('/').pop() ?? path).toLowerCase();
  if (name === 'options.txt' || name === 'optionsof.txt' || name === 'optionsshaders.txt') {
    return 'properties';
  }
  const extension = name.includes('.') ? name.slice(name.lastIndexOf('.') + 1) : '';
  switch (extension) {
    case 'toml':
      return 'toml';
    case 'json':
    case 'jsonc':
    case 'json5':
    case 'mcmeta':
      return 'json';
    case 'yml':
    case 'yaml':
      return 'yaml';
    case 'properties':
    case 'cfg':
    case 'ini':
    case 'conf':
      return 'properties';
    case 'js':
    case 'mjs':
    case 'cjs':
      return 'javascript';
    case 'ts':
      return 'typescript';
    default:
      return 'plain';
  }
}

/** Nome do formato mostrado no rodapé do editor (chave de `configs.formato.*`). */
export function formatKey(
  language: EditorLanguage,
): 'toml' | 'json' | 'yaml' | 'properties' | 'javascript' | 'texto' {
  switch (language) {
    case 'typescript':
    case 'javascript':
      return 'javascript';
    case 'plain':
      return 'texto';
    case 'toml':
    case 'json':
    case 'yaml':
    case 'properties':
      return language;
  }
}

/** Nome do arquivo, sem a pasta. */
export function baseName(path: string): string {
  return path.split('/').pop() ?? path;
}

/** `options.txt` e parentes: arquivos do jogo, mostrados com nome amigável na árvore. */
export function isGameOptions(path: string): boolean {
  return /^options(of|shaders)?\.txt$/i.test(path);
}

/** Um aviso contextual de um arquivo (SPEC T12). */
export type ConfigNotice = 'serverconfig' | 'reescrita' | 'neoforge' | 'options';

/** Versão do Minecraft como números ("1.20.1" → [1, 20, 1]); texto fora do padrão vira []. */
function versionParts(version: string | null): number[] {
  if (!version) {
    return [];
  }
  return version
    .split('.')
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part));
}

/** Se a versão do Minecraft é a 1.13 ou mais nova (`*-server.toml` existe a partir daí). */
export function isModernMinecraft(version: string | null): boolean {
  const [major = 0, minor = 0] = versionParts(version);
  return major > 1 || (major === 1 && minor >= 13);
}

export interface NoticeContext {
  path: string;
  origin: ConfigOrigin;
  /** Loader do pack (`forge`, `neoforge`, `fabric`…). */
  loader: string | null;
  minecraft: string | null;
}

/**
 * Os avisos que valem para o arquivo aberto (SPEC T12, "Avisos contextuais"):
 * - `*-server.toml` em `config/` do Forge/NeoForge 1.13+: vale por mundo;
 * - arquivos do Forge/NeoForge: o jogo pode reescrever e apagar comentários (e, no NeoForge,
 *   corrige valores fora da faixa e apaga chaves desconhecidas);
 * - `options.txt` no pack: substitui as preferências de quem já joga.
 */
export function noticesFor({ path, origin, loader, minecraft }: NoticeContext): ConfigNotice[] {
  const notices: ConfigNotice[] = [];
  const lower = path.toLowerCase();
  const forgeLike = loader === 'forge' || loader === 'neoforge';
  const topFolder = lower.split('/')[0] ?? '';
  const isSettingsFile =
    (topFolder === 'config' || topFolder === 'defaultconfigs') &&
    (lower.endsWith('.toml') || lower.endsWith('.cfg'));
  if (
    origin === 'pack' &&
    forgeLike &&
    isModernMinecraft(minecraft) &&
    lower.startsWith('config/') &&
    lower.endsWith('-server.toml')
  ) {
    notices.push('serverconfig');
  }
  if (forgeLike && isSettingsFile) {
    notices.push('reescrita');
    if (loader === 'neoforge') {
      notices.push('neoforge');
    }
  }
  if (origin === 'pack' && lower === 'options.txt') {
    notices.push('options');
  }
  return notices;
}
