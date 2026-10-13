/**
 * Regras puras do editor de scripts (SPEC T26; ARCHITECTURE §10.2): de quem é o arquivo, o
 * rótulo da versão ("KubeJS 6 · Forge 1.20.1"), o contexto do autocompletar de IDs e os erros
 * que o painel mostra. As mesmas regras de `warden-scripts::detect::ScriptFile`.
 */
import type {
  IdGroup,
  ScriptEnv,
  ScriptIds,
  ScriptProblem,
  Snippet,
  ScriptsInfo,
  ToolVersion,
} from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';

export type ScriptToolKind = 'kubejs' | 'crafttweaker';

/** A ferramenta dona do arquivo pela pasta e pela extensão (`null`: não é script). */
export function scriptToolOf(path: string): ScriptToolKind | null {
  const lower = path.replace(/^\.\//, '').toLowerCase();
  const kubejs = /^kubejs\/(startup|server|client)_scripts\/.+\.js$/.exec(lower);
  if (kubejs) {
    return 'kubejs';
  }
  if (/^scripts\/.+\.zs$/.test(lower)) {
    return 'crafttweaker';
  }
  return null;
}

/** A ferramenta do pack para o arquivo, se o pack a tem. */
export function toolFor(env: ScriptEnv, kind: ScriptToolKind): ToolVersion | null {
  return kind === 'kubejs' ? env.kubejs : env.crafttweaker;
}

const TOOL_NAMES: Record<ScriptToolKind, string> = {
  kubejs: 'KubeJS',
  crafttweaker: 'CraftTweaker',
};

/** "KubeJS 6" (ou só "KubeJS" quando a versão não pôde ser lida). */
export function toolName(kind: ScriptToolKind, tool: ToolVersion): string {
  return tool.major === null ? TOOL_NAMES[kind] : `${TOOL_NAMES[kind]} ${String(tool.major)}`;
}

/** "KubeJS 6 · Forge 1.20.1". */
export function toolLabel(
  kind: ScriptToolKind,
  tool: ToolVersion,
  loader: string | null,
  minecraft: string | null,
): string {
  const game = [loader ? loaderName(loader) : 'Minecraft', minecraft].filter(Boolean).join(' ');
  return `${toolName(kind, tool)} · ${game}`;
}

/** "JavaScript · KubeJS 6" ou "ZenScript · CraftTweaker 14" (rodapé do editor). */
export function formatLabel(kind: ScriptToolKind, tool: ToolVersion): string {
  return `${kind === 'kubejs' ? 'JavaScript' : 'ZenScript'} · ${toolName(kind, tool)}`;
}

/** Os trechos prontos da ferramenta, só os do tipo de script do arquivo (KubeJS). */
export function snippetsFor(info: ScriptsInfo, kind: ScriptToolKind, path: string): Snippet[] {
  if (kind === 'crafttweaker') {
    return info.crafttweakerSnippets;
  }
  const folder = /^kubejs\/(startup|server|client)_scripts\//i.exec(path)?.[1]?.toLowerCase();
  return info.kubejsSnippets.filter(
    (snippet) => snippet.scriptType === null || snippet.scriptType === folder,
  );
}

/** Tipo de um ID no autocompletar. */
export type IdKind = 'item' | 'block' | 'itemTag' | 'blockTag' | 'fluidTag' | 'recipe';

export interface IdOption {
  /** O texto inserido (`minecraft:stone` ou `#forge:ingots/iron`). */
  id: string;
  kind: IdKind;
  /** De onde veio ("Minecraft", nome do mod ou `kubejs-web`). */
  source: string;
}

const GROUP_FIELDS: readonly (readonly [keyof IdGroup, IdKind, boolean])[] = [
  ['items', 'item', false],
  ['blocks', 'block', false],
  ['itemTags', 'itemTag', true],
  ['blockTags', 'blockTag', true],
  ['fluidTags', 'fluidTag', true],
  ['recipes', 'recipe', false],
];

/** A lista plana de IDs, sem repetir o mesmo ID do mesmo tipo (o primeiro grupo ganha). */
export function flattenIds(ids: ScriptIds): IdOption[] {
  const seen = new Set<string>();
  const options: IdOption[] = [];
  for (const group of ids.groups) {
    for (const [field, kind, tag] of GROUP_FIELDS) {
      const values = group[field];
      if (!Array.isArray(values)) continue;
      for (const value of values) {
        const id = tag ? `#${value}` : value;
        const key = `${kind}:${id}`;
        if (seen.has(key)) continue;
        seen.add(key);
        options.push({ id, kind, source: group.source });
      }
    }
  }
  return options;
}

/** Onde o autocompletar de IDs se aplica, a partir do texto da linha até o cursor. */
export interface IdContext {
  /** Quantos caracteres antes do cursor o ID digitado ocupa. */
  length: number;
  /** O que já foi digitado do ID. */
  typed: string;
  /** Tipos aceitos nesse ponto. */
  kinds: readonly IdKind[];
}

const TAG_KINDS: readonly IdKind[] = ['itemTag', 'blockTag', 'fluidTag'];
const PLAIN_KINDS: readonly IdKind[] = ['item', 'block', 'recipe'];
const ITEMS_AND_BLOCKS: readonly IdKind[] = ['item', 'block'];

/**
 * KubeJS: dentro de uma string (`'minecraft:copper_`, `"#forge:ingots/`). CraftTweaker: dentro
 * de um colchete angular (`<item:minecraft:copp`, `<tag:items:forge:ingots/`, `<block:…`,
 * `<minecraft:stone` no 1.12.2).
 */
export function idContext(before: string, kind: ScriptToolKind): IdContext | null {
  if (kind === 'kubejs') {
    const match = /(['"`])(#?[a-z0-9_.-]*(?::[a-z0-9_./-]*)?)$/.exec(before);
    if (!match?.[2]) return null;
    // A aspa precisa abrir uma string (número par de aspas iguais antes dela).
    const quote = match[1] ?? "'";
    const prefix = before.slice(0, before.length - match[2].length - 1);
    const count = prefix.split(quote).length - 1;
    if (count % 2 !== 0) return null;
    const typed = match[2];
    return { length: typed.length, typed, kinds: typed.startsWith('#') ? TAG_KINDS : PLAIN_KINDS };
  }
  const bracket = /<([a-z0-9_.:/-]*)$/.exec(before);
  if (!bracket) return null;
  const inside = bracket[1] ?? '';
  const prefixed = /^(item|block|fluid|tag:(items|blocks|fluids)):(.*)$/.exec(inside);
  if (prefixed) {
    const typed = prefixed[3] ?? '';
    const what = prefixed[1] ?? '';
    const kinds: readonly IdKind[] =
      what === 'item'
        ? ['item']
        : what === 'block'
          ? ['block']
          : what === 'tag:items'
            ? ['itemTag']
            : what === 'tag:blocks'
              ? ['blockTag']
              : what === 'tag:fluids'
                ? ['fluidTag']
                : [];
    return { length: typed.length, typed, kinds };
  }
  // 1.12.2: `<minecraft:stone>` direto. Sem `:` ainda, pode ser o começo de `item:`.
  if (!inside.includes(':') && inside.length < 2) return null;
  return { length: inside.length, typed: inside, kinds: ITEMS_AND_BLOCKS };
}

/** Quantas sugestões o autocompletar mostra no máximo. */
export const MAX_ID_OPTIONS = 200;

/** As sugestões para o que foi digitado: começam com o texto antes, depois as que o contêm. */
export function matchIds(options: readonly IdOption[], context: IdContext): IdOption[] {
  const wanted = context.typed.replace(/^#/, '').toLowerCase();
  const tagged = context.typed.startsWith('#');
  const accepted = new Set(context.kinds);
  const starts: IdOption[] = [];
  const contains: IdOption[] = [];
  for (const option of options) {
    if (!accepted.has(option.kind)) continue;
    const isTag = option.id.startsWith('#');
    if (tagged && !isTag) continue;
    const bare = isTag ? option.id.slice(1) : option.id;
    const lower = bare.toLowerCase();
    if (lower.startsWith(wanted)) {
      starts.push(option);
    } else if (wanted.length >= 2 && lower.includes(wanted)) {
      contains.push(option);
    }
    if (starts.length >= MAX_ID_OPTIONS) break;
  }
  return [...starts, ...contains].slice(0, MAX_ID_OPTIONS);
}

/** O texto inserido: no CraftTweaker, sem o `#` (o colchete já diz que é tag). */
export function insertText(option: IdOption, kind: ScriptToolKind): string {
  return kind === 'crafttweaker' ? option.id.replace(/^#/, '') : option.id;
}

/** Os erros do painel: os da ferramenta do arquivo, os do arquivo aberto primeiro. */
export function problemsFor(
  problems: readonly ScriptProblem[],
  kind: ScriptToolKind,
  path: string,
): ScriptProblem[] {
  const own = (problem: ScriptProblem) =>
    kind === 'kubejs'
      ? (problem.file?.startsWith('kubejs/') ?? problem.logFile?.startsWith('logs/kubejs/') ?? true)
      : (problem.file?.startsWith('scripts/') ??
        problem.logFile?.toLowerCase().includes('crafttweaker') ??
        false);
  const list = problems.filter(own);
  const lower = path.toLowerCase();
  return [
    ...list.filter((problem) => problem.file?.toLowerCase() === lower),
    ...list.filter((problem) => problem.file?.toLowerCase() !== lower),
  ];
}

/** As linhas com erro no arquivo aberto (para marcar no editor). */
export function errorLines(problems: readonly ScriptProblem[], path: string): number[] {
  const lower = path.toLowerCase();
  const lines = new Set<number>();
  for (const problem of problems) {
    if (problem.level === 'error' && problem.line && problem.file?.toLowerCase() === lower) {
      lines.add(problem.line);
    }
  }
  return [...lines].sort((a, b) => a - b);
}

/** Nome do arquivo sem as pastas ("receitas.js"). */
export function fileName(path: string): string {
  return path.split('/').pop() ?? path;
}

/**
 * Um caminho de script do tipo que o botão "Recarregar scripts" da tela do teste recarrega: só
 * a pasta conta para escolher o comando (scripts de servidor no KubeJS; os do CraftTweaker).
 */
export function reloadTargetPath(env: ScriptEnv): string | null {
  if (env.kubejs) return 'kubejs/server_scripts/scripts.js';
  if (env.crafttweaker) return 'scripts/scripts.zs';
  return null;
}
