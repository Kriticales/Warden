/**
 * Regras da tela de chaves (Configurações → Chaves e contas e Sensor Sculk; ADR-0069 e
 * ADR-0070), sem React: a situação de cada chave, o provedor das conversas novas e quem assume
 * quando a chave do provedor em uso sai.
 */
import type {
  AiProvider,
  AiSettingsView,
  KeyOverview,
  KeysOverview,
  SecretKind,
} from '../../../lib/ipc/bindings';
import { dayRelation, formatDate, formatTime } from '../../../lib/format';

/** As chaves de "Mods e publicação", na ordem da tela. */
export const ACCOUNT_KINDS = ['curseforge', 'github'] as const satisfies readonly SecretKind[];

/**
 * A situação de uma chave na tela:
 * - `none`: sem chave no cofre;
 * - `testing`: o **Testar** está rodando;
 * - `refused`: o serviço recusou a chave no último teste (o Sculk pula o provedor);
 * - `ok`: o serviço aceitou a chave no último teste;
 * - `untested`: guardada, ainda sem teste (o Sculk usa normalmente).
 */
export type KeyState = 'none' | 'testing' | 'refused' | 'ok' | 'untested';

export function keyState(key: KeyOverview | undefined, testing: boolean): KeyState {
  if (!key?.configured) return 'none';
  if (testing) return 'testing';
  if (key.check?.outcome === 'refused') return 'refused';
  if (key.check?.outcome === 'working') return 'ok';
  return 'untested';
}

/** O GitHub chama a chave de token (glossário, QUALITY §8). */
export function isToken(kind: SecretKind): boolean {
  return kind === 'github';
}

/** O provedor do Sculk desta chave, se for de um. */
export function providerOf(kind: SecretKind): AiProvider | null {
  return kind === 'gemini' || kind === 'openai' || kind === 'anthropic' ? kind : null;
}

export function keyOf(overview: KeysOverview | undefined, kind: SecretKind) {
  return overview?.keys.find((key) => key.kind === kind);
}

/** Quantas chaves foram recusadas no último teste. */
export function refusedCount(overview: KeysOverview | undefined): number {
  return overview?.keys.filter((key) => keyState(key, false) === 'refused').length ?? 0;
}

/** Se o provedor pode abrir uma conversa nova: chave no cofre, não recusada (a mesma do Rust). */
export function isUsable(view: AiSettingsView, provider: AiProvider): boolean {
  const item = view.providers.find((entry) => entry.provider === provider);
  return item !== undefined && item.keyConfigured && !item.keyRefused;
}

/** O que o Sculk faz com o provedor na conversa nova. */
export type ProviderRole = 'use' | 'reserve' | 'noKey' | 'refused';

export function providerRole(view: AiSettingsView, provider: AiProvider): ProviderRole {
  const item = view.providers.find((entry) => entry.provider === provider);
  if (!item?.keyConfigured) return 'noKey';
  if (item.keyRefused) return 'refused';
  return view.provider === provider ? 'use' : 'reserve';
}

/** O primeiro da ordem com a chave funcionando, ou `null` (nenhum). */
export function firstUsable(view: AiSettingsView): AiProvider | null {
  return view.order.find((provider) => isUsable(view, provider)) ?? null;
}

/**
 * Quem assume as conversas novas se a chave de `removed` sair: `undefined` quando `removed` não
 * é o provedor em uso (nada muda); `null` quando nenhum outro tem chave funcionando.
 */
export function successorAfterRemoval(
  view: AiSettingsView,
  removed: AiProvider,
): AiProvider | null | undefined {
  if (firstUsable(view) !== removed) return undefined;
  return view.order.find((provider) => provider !== removed && isUsable(view, provider)) ?? null;
}

/** Move o item `index` em `delta` posições (fora dos limites, a ordem fica igual). */
export function moveItem<T>(items: readonly T[], index: number, delta: number): T[] {
  const target = index + delta;
  const next = [...items];
  if (target < 0 || target >= items.length || index < 0 || index >= items.length) return next;
  const [item] = next.splice(index, 1);
  if (item !== undefined) next.splice(target, 0, item);
  return next;
}

/** Partes de "hoje às 21:40", "ontem às 9:05" ou "em 03/10/2026 às 14:00". */
export function whenParts(
  iso: string,
  now: number,
): { relation: 'today' | 'yesterday' | 'other'; hora: string; data: string } | null {
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return null;
  return { relation: dayRelation(ms, now), hora: formatTime(ms), data: formatDate(ms) };
}
