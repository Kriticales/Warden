/**
 * O que a coluna de conversas e a gaveta Detalhes do Sensor Sculk derivam de cada conversa
 * (protótipo `design/bug-01/sensor-sculk.html`): o selo da conversa, o grupo por data, os avisos
 * da conversa e o modo e o modelo de cada pergunta (ADR-0064).
 */
import type {
  AiConversationState,
  AiConversationSummary,
  AiConversationView,
  AiProvider,
  ConversationTurn,
  FallbackReason,
  SearchMode,
} from '../../../lib/ipc/bindings';
import { dayRelation } from '../../../lib/format';

/** O selo da conversa na lista. */
export type ConversationTag =
  'lendo' | 'respondida' | 'esperando' | 'aplicada' | 'erro' | 'interrompida';

/**
 * Lendo enquanto o Sculk responde; Esperando você quando há proposta para decidir; Aplicada
 * quando alguma proposta foi aplicada e nenhuma espera; senão Respondida (ou o erro).
 */
export function conversationTag(summary: AiConversationSummary): ConversationTag {
  const byState: Record<AiConversationState, ConversationTag | null> = {
    running: 'lendo',
    failed: 'erro',
    interrupted: 'interrompida',
    answered: null,
  };
  const fixed = byState[summary.state];
  if (fixed) return fixed;
  if (summary.pendingProposals > 0) return 'esperando';
  if (summary.appliedProposals > 0) return 'aplicada';
  return 'respondida';
}

/** Os grupos da lista, na ordem. */
export const GROUPS = ['hoje', 'ontem', 'semana', 'antes'] as const;
export type ConversationGroup = (typeof GROUPS)[number];

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

/** O grupo pela última atividade da conversa. */
export function groupOf(atMs: number, now: number = Date.now()): ConversationGroup {
  const relation = dayRelation(atMs, now);
  if (relation === 'today') return 'hoje';
  if (relation === 'yesterday') return 'ontem';
  return now - atMs < WEEK_MS ? 'semana' : 'antes';
}

/** Sem acentos e em minúsculas, para a busca. */
export function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();
}

/** O modelo que respondeu (ou vai responder) a pergunta. */
export function turnModel(turn: ConversationTurn, conversationModel: string): string {
  return turn.fallbackModel ?? turn.model ?? conversationModel;
}

/** O modo das perguntas da conversa: cada um com quantas perguntas o usaram. */
export function modesUsed(turns: readonly ConversationTurn[]): Record<SearchMode, number> {
  const used: Record<SearchMode, number> = { quick: 0, deep: 0 };
  for (const turn of turns) {
    if (turn.mode) used[turn.mode] += 1;
  }
  return used;
}

/** O modo que o campo de pergunta propõe: o da última pergunta, ou "Procurar a fundo". */
export function lastMode(turns: readonly ConversationTurn[]): SearchMode {
  return turns.findLast((turn) => turn.mode)?.mode ?? 'deep';
}

/** Um aviso da conversa (gaveta Detalhes; o número aparece no botão). */
export type ConversationNotice =
  | { kind: 'mudou' }
  | { kind: 'troca'; reason: FallbackReason; model: string; count: number }
  | { kind: 'planoGratuito' };

export function noticesOf(view: AiConversationView, provider: AiProvider): ConversationNotice[] {
  const notices: ConversationNotice[] = [];
  const changes = view.packChanges;
  if (changes.updated + changes.added + changes.removed > 0 || changes.other) {
    notices.push({ kind: 'mudou' });
  }
  const swaps = new Map<string, { reason: FallbackReason; model: string; count: number }>();
  for (const turn of view.turns) {
    if (!turn.fallbackModel) continue;
    const reason = turn.fallbackReason ?? 'quota';
    const key = `${reason}:${turn.fallbackModel}`;
    const found = swaps.get(key);
    if (found) found.count += 1;
    else swaps.set(key, { reason, model: turn.fallbackModel, count: 1 });
  }
  for (const swap of swaps.values()) notices.push({ kind: 'troca', ...swap });
  if (provider === 'gemini') notices.push({ kind: 'planoGratuito' });
  return notices;
}
