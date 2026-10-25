/**
 * Regra do diálogo Adicionar itens (SPEC T09; testada em `model.test.ts`): o que entra no pack
 * a partir do plano (`add_plan`) e das escolhas do usuário, o pedido de gravação (`add_apply`),
 * a árvore por mod escolhido, a situação de cada linha e a conta por papel (esboço
 * `design/bug-01/adicionar-itens.html`, opção 4).
 *
 * - **Escolhido:** entra se continuar marcado. Se já está no pack por outra fonte, só entra
 *   quando o usuário escolhe substituir (manter os dois faria o jogo travar).
 * - **Obrigatória:** entra quando algum item que a pede entra, salvo se o usuário desmarcar.
 * - **Opcional:** só entra se o usuário marcar e algum item para quem ela é opcional entrar.
 *
 * O fecho é transitivo: a obrigatória de uma opcional só entra se a opcional entrar; a de uma
 * obrigatória desmarcada também sai (a não ser que outro item que entra a peça).
 */
import type {
  AddApplyRequest,
  AddPlan,
  PlanConflict,
  PlanInstalled,
  PlanMissing,
  PlanNode,
  SourceId,
} from '../../../lib/ipc/bindings';

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

/** Só as chaves dos itens que entram (para "Pedida por X", sem quem foi desmarcado). */
export function onlyIncluded(keys: readonly string[], included: ReadonlySet<string>): string[] {
  return keys.filter((key) => included.has(key));
}

/** Os nomes dos itens do plano pelas chaves (para "Pedida por X e Y"). */
export function titlesOf(plan: AddPlan, keys: readonly string[]): string[] {
  const byKey = new Map<string, string>(plan.nodes.map((node) => [node.key, node.title]));
  for (const installed of plan.installed) byKey.set(installed.key, installed.title);
  for (const missing of plan.missing)
    if (!byKey.has(missing.key)) byKey.set(missing.key, missing.title);
  return keys.map((key) => byKey.get(key) ?? key);
}

/** Lista legível em pt-BR: "A", "A e B", "A, B e C". */
export function joinNames(names: readonly string[]): string {
  return new Intl.ListFormat('pt-BR', { style: 'long', type: 'conjunction' }).format(names);
}

/** A conta por papel: o título, o botão e o rodapé mostram os mesmos números. */
export interface RoleCounts {
  chosen: number;
  required: number;
  optional: number;
  total: number;
}

export function roleCounts(plan: AddPlan, included: ReadonlySet<string>): RoleCounts {
  const counts: RoleCounts = { chosen: 0, required: 0, optional: 0, total: 0 };
  for (const node of plan.nodes) {
    if (!included.has(node.key)) continue;
    counts[node.role] += 1;
    counts.total += 1;
  }
  return counts;
}

/** Uma linha da árvore: um nó do plano, um item que já está no pack ou um sem versão. */
export type TreeItem =
  | { kind: 'node'; key: string; node: PlanNode }
  | { kind: 'installed'; key: string; item: PlanInstalled }
  | { kind: 'missing'; key: string; item: PlanMissing };

export interface TreeRow {
  item: TreeItem;
  /** 0: o mod escolhido (ou uma linha de "Em comum"); 1, 2…: penduradas. */
  depth: number;
  /** Última pendurada do seu galho (o traço da árvore termina nela). */
  last: boolean;
}

/** A árvore do diálogo (esboço `adicionar-itens.html`, opção 4). */
export interface PlanTree {
  /** Um galho por mod escolhido, na ordem pedida. */
  branches: TreeRow[][];
  /** Pedidas por mais de um mod: aparecem uma vez só, com o que penduram nelas. */
  common: TreeRow[][];
}

/** Quem pede um item (as chaves dos pais na árvore). */
function parentsOf(item: TreeItem): readonly string[] {
  switch (item.kind) {
    case 'node':
      return item.node.role === 'optional' ? item.node.optionalFor : item.node.requiredBy;
    case 'installed':
    case 'missing':
      return item.item.requiredBy;
  }
}

function isRoot(item: TreeItem): boolean {
  switch (item.kind) {
    case 'node':
      return item.node.role === 'chosen';
    case 'installed':
      return item.item.chosen;
    case 'missing':
      return item.item.role === 'chosen';
  }
}

/** Ordem das penduradas: obrigatórias, opcionais, sem versão e as que já estão no pack. */
function rank(item: TreeItem): number {
  if (item.kind === 'node') return item.node.role === 'required' ? 0 : 1;
  return item.kind === 'missing' ? 2 : 3;
}

/**
 * Monta a árvore: cada mod escolhido vira um galho; o que só ele pede fica pendurado nele (e o
 * que só a pendurada pede, pendurado nela); o que mais de um item pede vai para "Em comum", uma
 * vez só. Nada aparece duas vezes, e um ciclo declarado não trava a montagem.
 */
export function planTree(plan: AddPlan): PlanTree {
  const items: TreeItem[] = [
    ...plan.nodes.map((node): TreeItem => ({ kind: 'node', key: node.key, node })),
    ...plan.missing.map((item): TreeItem => ({ kind: 'missing', key: item.key, item })),
    ...plan.installed.map((item): TreeItem => ({ kind: 'installed', key: item.key, item })),
  ];
  const known = new Set(items.map((item) => item.key));
  const placed = new Set<string>();
  const childrenOf = (key: string) =>
    items
      .filter((item) => !isRoot(item) && !placed.has(item.key))
      .filter((item) => {
        const parents = parentsOf(item);
        return parents.length === 1 && parents[0] === key;
      })
      .sort((a, b) => rank(a) - rank(b));
  const grow = (item: TreeItem, depth: number, last: boolean, rows: TreeRow[]) => {
    placed.add(item.key);
    rows.push({ item, depth, last });
    const children = childrenOf(item.key);
    children.forEach((child, index) => {
      if (!placed.has(child.key)) grow(child, depth + 1, index === children.length - 1, rows);
    });
  };
  const branch = (item: TreeItem) => {
    const rows: TreeRow[] = [];
    grow(item, 0, false, rows);
    return rows;
  };
  const branches = items.filter(isRoot).map(branch);
  // "Em comum": pedidas por mais de um item, ou por quem não está no plano.
  const common = items
    .filter((item) => !placed.has(item.key))
    .filter((item) => {
      const parents = parentsOf(item);
      return parents.length !== 1 || !known.has(parents[0] ?? '');
    })
    .sort((a, b) => rank(a) - rank(b))
    .map(branch);
  // O que sobrou pende de algo que ficou num ciclo: entra em "Em comum" também.
  for (const item of items) if (!placed.has(item.key)) common.push(branch(item));
  return { branches, common };
}

/** Situação de uma linha, uma só (a mais importante), para a linha nunca crescer. */
export type RowTag =
  | 'beta'
  | 'alpha'
  | 'semVersao'
  | 'incompativel'
  | 'jaNoPack'
  | 'substitui'
  | 'desmarcado'
  | 'naoEntra'
  | 'jogoPodeNaoAbrir'
  | 'versaoIncompativel';

export interface RowState {
  checked: boolean;
  disabled: boolean;
  tag: RowTag | null;
  /** Por que fica de fora (para o detalhe). */
  out: 'semVersao' | 'opcional' | 'manteve' | 'desmarcou' | 'pedidoDesmarcado' | null;
}

/** As incompatibilidades em que o item entra (o outro lado já está no pack ou também entra). */
export function activeConflicts(plan: AddPlan, included: ReadonlySet<string>): PlanConflict[] {
  return plan.conflicts.filter(
    (conflict) =>
      included.has(conflict.item.key) && (conflict.withPack || included.has(conflict.other.key)),
  );
}

export function rowState(
  plan: AddPlan,
  node: PlanNode,
  included: ReadonlySet<string>,
  choices: DependencyChoices,
): RowState {
  const checked = included.has(node.key);
  const state: RowState = { checked, disabled: false, tag: null, out: null };
  if (node.role === 'chosen') {
    const duplicate = plan.duplicates.some((item) => item.key === node.key);
    if (duplicate && !choices.replace.has(node.key)) {
      return { checked, disabled: true, tag: 'jaNoPack', out: 'manteve' };
    }
    if (duplicate) state.tag = 'substitui';
    if (!checked) return { ...state, tag: 'desmarcado', out: 'desmarcou' };
  } else {
    const parents = node.role === 'optional' ? node.optionalFor : node.requiredBy;
    const asked = parents.some((key) => included.has(key));
    if (!asked) return { checked, disabled: true, tag: 'naoEntra', out: 'pedidoDesmarcado' };
    if (node.role === 'required' && choices.unchecked.has(node.key)) {
      return { ...state, tag: 'jogoPodeNaoAbrir', out: 'desmarcou' };
    }
    if (!checked) state.out = 'opcional';
  }
  if (activeConflicts(plan, included).some((conflict) => conflict.item.key === node.key)) {
    state.tag = 'incompativel';
  } else if (!node.compatible) {
    state.tag ??= 'versaoIncompativel';
  } else if (node.outsideChannel && node.channel !== 'release') {
    state.tag ??= node.channel;
  }
  return state;
}

/** Um item que pede uma decisão antes de adicionar (a linha no topo da árvore). */
export interface Attention {
  key: string;
  title: string;
  reason: 'incompativel' | 'duplicado' | 'obrigatoria';
  /** O outro lado (incompatibilidade) ou a fonte do que já está no pack (duplicado). */
  other: string | null;
  source: SourceId | null;
  severity: 'danger' | 'warn';
}

export function attentionItems(
  plan: AddPlan,
  included: ReadonlySet<string>,
  choices: DependencyChoices,
): Attention[] {
  const title = (key: string) => titlesOf(plan, [key])[0] ?? key;
  const items: Attention[] = [];
  for (const conflict of activeConflicts(plan, included)) {
    items.push({
      key: conflict.item.key,
      title: conflict.item.title,
      reason: 'incompativel',
      other: conflict.other.title,
      source: null,
      severity: 'danger',
    });
  }
  for (const duplicate of plan.duplicates) {
    if (choices.replace.has(duplicate.key)) continue;
    items.push({
      key: duplicate.key,
      title: title(duplicate.key),
      reason: 'duplicado',
      other: null,
      source: duplicate.existingSource,
      severity: 'warn',
    });
  }
  for (const node of plan.nodes) {
    if (node.role !== 'required' || !choices.unchecked.has(node.key)) continue;
    if (!node.requiredBy.some((key) => included.has(key))) continue;
    items.push({
      key: node.key,
      title: node.title,
      reason: 'obrigatoria',
      other: null,
      source: null,
      severity: 'danger',
    });
  }
  return items;
}

/** Os itens que entram e não informam o lado (o aviso aparece uma vez, no topo). */
export function unknownSide(plan: AddPlan, included: ReadonlySet<string>): PlanNode[] {
  return plan.nodes.filter((node) => included.has(node.key) && node.sideNote === 'unknown');
}

/** Tira ou devolve um item do que entra, qualquer que seja o papel dele. */
export function setIncluded(
  plan: AddPlan,
  choices: DependencyChoices,
  key: string,
  on: boolean,
): DependencyChoices {
  const node = plan.nodes.find((item) => item.key === key);
  if (!node) return choices;
  const toggle = (set: ReadonlySet<string>, add: boolean) => {
    const next = new Set(set);
    if (add) next.add(key);
    else next.delete(key);
    return next;
  };
  if (node.role === 'optional') return { ...choices, optional: toggle(choices.optional, on) };
  return { ...choices, unchecked: toggle(choices.unchecked, !on) };
}

/** O item que o detalhe mostra ao abrir: o primeiro que pede decisão, ou o primeiro escolhido. */
export function firstCurrent(plan: AddPlan, attention: readonly Attention[]): string | null {
  return (
    attention[0]?.key ??
    plan.nodes.find((node) => node.role === 'chosen')?.key ??
    plan.nodes[0]?.key ??
    plan.missing[0]?.key ??
    plan.installed[0]?.key ??
    null
  );
}

/** "Pedida por" curto quando vale para todos os escolhidos (evita uma lista enorme). */
export function byAllChosen(plan: AddPlan, keys: readonly string[]): boolean {
  const chosen = plan.nodes.filter((node) => node.role === 'chosen').map((node) => node.key);
  return (
    keys.length > 2 && keys.length === chosen.length && keys.every((key) => chosen.includes(key))
  );
}
