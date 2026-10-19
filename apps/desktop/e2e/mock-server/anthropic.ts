/**
 * Claude (Anthropic) simulado dos E2E (D-17): `GET v1/models`, `POST v1/messages/count_tokens` e
 * `POST v1/messages`, com as regras da API Messages que importam para o laço (as mesmas do
 * simulado da `warden-ai`):
 *
 * - a chave vem só no cabeçalho `x-api-key` (nunca na URL), com o `anthropic-version`;
 * - a escolha forçada (`tool_choice` `any` ou `tool`) é recusada, como no Sonnet 5.5 e no Opus 5.5;
 * - cada mensagem do assistente é **idêntica** a uma resposta que o simulado mandou (o bloco de
 *   raciocínio e a assinatura intactos); depois de um `tool_use` vem uma mensagem do usuário que
 *   começa com um `tool_result` para cada `id`; os papéis se alternam.
 *
 * O roteiro é o de `ia-roteiro.ts`. Os corpos recebidos ficam guardados para o teste comparar
 * com o que a tela mostra (`GET /__anthropic`).
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

import { canonical, nextCalls, parseResult } from './ia-roteiro';

/** A chave que os E2E gravam no cofre de teste. */
export const ANTHROPIC_E2E_KEY = 'chave-e2e-da-anthropic-simulada';

/** O modelo "Mais econômico" da lista abaixo (o Sonnet mais novo). */
export const ANTHROPIC_E2E_MODEL = 'claude-sonnet-9-5';

interface AnthropicState {
  /** Corpos de `v1/messages` aceitos, como chegaram. */
  generate: string[];
  /** Corpos de `v1/messages/count_tokens`. */
  countTokens: string[];
  /** Recusas (raciocínio alterado, escolha forçada, chave errada). */
  refusals: string[];
}

type Json = Record<string, unknown>;

let state: AnthropicState = { generate: [], countTokens: [], refusals: [] };
let emitted: Json[] = [];
let counter = 0;

export function anthropicState(): AnthropicState {
  return state;
}

export function resetAnthropic(): void {
  state = { generate: [], countTokens: [], refusals: [] };
  emitted = [];
}

function blocks(message: Json | undefined): Json[] {
  return Array.isArray(message?.content) ? (message.content as Json[]) : [];
}

/** As regras da API Messages com o Sonnet 5.5 e o Opus 5.5. */
function validate(body: Json): string | null {
  const choice = (body.tool_choice as Json | undefined)?.type;
  if (choice === 'any' || choice === 'tool') {
    return 'tool_choice: type "tool" and "any" are not supported for this model.';
  }
  if (typeof body.max_tokens !== 'number') return 'max_tokens: Field required';
  const messages = Array.isArray(body.messages) ? (body.messages as Json[]) : [];
  for (const [i, message] of messages.entries()) {
    const expected = i % 2 === 0 ? 'user' : 'assistant';
    if (message.role !== expected) return `messages.${String(i)}: roles must alternate`;
    if (message.role !== 'assistant') continue;
    if (!emitted.some((sent) => canonical(sent) === canonical(message))) {
      return `messages.${String(i)}.content.0: Invalid \`signature\` in \`thinking\` block`;
    }
    const ids = blocks(message)
      .filter((block) => block.type === 'tool_use')
      .map((block) => block.id);
    const results: unknown[] = [];
    for (const block of blocks(messages[i + 1])) {
      if (block.type !== 'tool_result') break;
      results.push(block.tool_use_id);
    }
    if (ids.some((id) => !results.includes(id))) {
      return `messages.${String(i)}: \`tool_use\` ids were found without \`tool_result\` blocks immediately after`;
    }
  }
  return null;
}

/** A última pergunta, quantas respostas do modelo vieram depois dela e os últimos resultados. */
function turn(messages: Json[]): { question: string; step: number; results: unknown[] } {
  let question = '';
  let step = 0;
  for (const message of messages) {
    const text = blocks(message).find((block) => block.type === 'text')?.text;
    if (message.role === 'user' && typeof text === 'string') {
      question = text;
      step = 0;
    } else if (message.role === 'assistant') {
      step += 1;
    }
  }
  const last = messages.at(-1);
  const results = blocks(last)
    .filter((block) => block.type === 'tool_result')
    .map((block) => parseResult(block.content));
  return { question, step, results };
}

function reply(messages: Json[]): Json {
  const current = turn(messages);
  counter += 1;
  const content: Json[] = [
    { type: 'thinking', thinking: '', signature: `ZTJlLWFzc2luYXR1cmEt${String(counter)}` },
  ];
  for (const [name, input] of nextCalls(current.question, current.step, current.results)) {
    counter += 1;
    content.push({ type: 'tool_use', id: `toolu_${String(counter)}`, name, input });
  }
  return {
    id: `msg_${String(counter)}`,
    type: 'message',
    role: 'assistant',
    model: ANTHROPIC_E2E_MODEL,
    content,
    stop_reason: 'tool_use',
    stop_sequence: null,
    usage: {
      input_tokens: 100,
      cache_creation_input_tokens: 0,
      cache_read_input_tokens: 2000,
      output_tokens: 60,
    },
  };
}

function send(response: ServerResponse, status: number, body: unknown): void {
  response
    .writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
    .end(JSON.stringify(body));
}

function error(response: ServerResponse, status: number, type: string, message: string): void {
  send(response, status, { type: 'error', error: { type, message } });
}

/** Atende `/anthropic/…` (o app aponta `WARDEN_API_BASE_ANTHROPIC` para cá). */
export function serveAnthropic(
  request: IncomingMessage,
  response: ServerResponse,
  path: string,
): void {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    if (path.includes(ANTHROPIC_E2E_KEY) || request.headers['x-api-key'] !== ANTHROPIC_E2E_KEY) {
      state.refusals.push('chave');
      error(response, 401, 'authentication_error', 'invalid x-api-key');
      return;
    }
    if (request.headers['anthropic-version'] !== '2023-06-01') {
      state.refusals.push('anthropic-version');
      error(response, 400, 'invalid_request_error', 'anthropic-version: header is required');
      return;
    }
    const route = path.split('?')[0] ?? '';
    if (request.method === 'GET' && route.endsWith('/v1/models')) {
      const ids = ['claude-haiku-9-5', 'claude-opus-9-5', 'claude-sonnet-9-5', 'claude-fable-9'];
      send(response, 200, {
        data: ids.map((id) => ({ type: 'model', id, display_name: id })),
        has_more: false,
        first_id: ids[0],
        last_id: ids.at(-1),
      });
      return;
    }
    const body = Buffer.concat(chunks).toString('utf8');
    if (route.endsWith('/v1/messages/count_tokens')) {
      state.countTokens.push(body);
      send(response, 200, { input_tokens: Math.ceil(body.length / 4) });
      return;
    }
    if (!route.endsWith('/v1/messages')) {
      error(response, 404, 'not_found_error', 'Not found');
      return;
    }
    const parsed = JSON.parse(body) as Json;
    const refused = validate(parsed);
    if (refused) {
      state.refusals.push(refused);
      error(response, 400, 'invalid_request_error', refused);
      return;
    }
    state.generate.push(body);
    const answer = reply(Array.isArray(parsed.messages) ? (parsed.messages as Json[]) : []);
    emitted.push({ role: 'assistant', content: answer.content });
    send(response, 200, answer);
  });
}
