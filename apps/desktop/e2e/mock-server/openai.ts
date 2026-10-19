/**
 * OpenAI simulada dos E2E (D-16): `GET v1/models` e `POST v1/responses`, com as regras da
 * Responses API sem estado que importam para o laço (as mesmas do simulado da `warden-ai`):
 *
 * - a chave vem só no cabeçalho `Authorization: Bearer …` (nunca na URL);
 * - `store: false` e o raciocínio criptografado (`include`), como o Warden promete;
 * - cada item do modelo em `input` vem na sequência inteira e **idêntica** a uma resposta que o
 *   simulado mandou (o raciocínio criptografado intacto), seguida de um `function_call_output`
 *   por chamada, com o mesmo `call_id`.
 *
 * O roteiro é o de `ia-roteiro.ts`. Os corpos recebidos ficam guardados para o teste comparar
 * com o que a tela mostra (`GET /__openai`).
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

import { canonical, nextCalls, parseResult } from './ia-roteiro';

/** A chave que os E2E gravam no cofre de teste. */
export const OPENAI_E2E_KEY = 'chave-e2e-da-openai-simulada';

/** O modelo "Mais econômico" da lista abaixo (a regra escolhe `gpt-9-luna` e `gpt-9-sol`). */
export const OPENAI_E2E_MODEL = 'gpt-9-luna';

interface OpenAiState {
  /** Corpos de `v1/responses` aceitos, como chegaram. */
  generate: string[];
  /** Recusas (histórico alterado, chave errada). */
  refusals: string[];
}

type Json = Record<string, unknown>;

let state: OpenAiState = { generate: [], refusals: [] };
let emitted: Json[][] = [];
let counter = 0;

export function openaiState(): OpenAiState {
  return state;
}

export function resetOpenai(): void {
  state = { generate: [], refusals: [] };
  emitted = [];
}

function kind(item: Json | undefined): string {
  return typeof item?.type === 'string' ? item.type : '';
}

function isUser(item: Json | undefined): boolean {
  return kind(item) === 'message' && item?.role === 'user';
}

/** As regras da Responses API sem estado com modelos de raciocínio. */
function validate(body: Json): string | null {
  if (body.store !== false) return 'store precisa ser false';
  const include = body.include;
  if (!Array.isArray(include) || !include.includes('reasoning.encrypted_content')) {
    return 'include sem reasoning.encrypted_content';
  }
  const input = Array.isArray(body.input) ? (body.input as Json[]) : [];
  let i = 0;
  while (i < input.length) {
    const item = input[i];
    if (isUser(item)) {
      i += 1;
      continue;
    }
    const output = emitted.find(
      (sent) =>
        sent.length <= input.length - i &&
        sent.every((part, k) => canonical(part) === canonical(input[i + k])),
    );
    if (!output) return `Item ${String(i)} do modelo foi alterado ou veio sem o raciocínio.`;
    i += output.length;
    for (const call of output.filter((part) => kind(part) === 'function_call')) {
      const next = input[i];
      if (kind(next) !== 'function_call_output' || next?.call_id !== call.call_id) {
        return `No tool output found for function call ${String(call.call_id)}.`;
      }
      i += 1;
    }
  }
  return null;
}

/** A última pergunta, quantas respostas do modelo vieram depois dela e os últimos resultados. */
function turn(input: Json[]): { question: string; step: number; results: unknown[] } {
  let question = '';
  let step = 0;
  let results: unknown[] = [];
  for (const item of input) {
    if (isUser(item)) {
      question = typeof item.content === 'string' ? item.content : '';
      step = 0;
      results = [];
    } else if (kind(item) === 'reasoning') {
      step += 1;
      results = [];
    } else if (kind(item) === 'function_call_output') {
      results.push(parseResult(item.output));
    }
  }
  return { question, step, results };
}

function reply(input: Json[]): Json {
  const current = turn(input);
  counter += 1;
  const output: Json[] = [
    {
      type: 'reasoning',
      id: `rs_${String(counter)}`,
      summary: [],
      encrypted_content: `ZTJlLXJhY2lvY2luaW8t${String(counter)}`,
    },
  ];
  for (const [name, args] of nextCalls(current.question, current.step, current.results)) {
    counter += 1;
    output.push({
      type: 'function_call',
      id: `fc_${String(counter)}`,
      call_id: `call_${String(counter)}`,
      name,
      arguments: JSON.stringify(args),
      status: 'completed',
    });
  }
  return {
    id: `resp_${String(counter)}`,
    object: 'response',
    status: 'completed',
    model: OPENAI_E2E_MODEL,
    output,
    usage: {
      input_tokens: 2100,
      input_tokens_details: { cached_tokens: 1024 },
      output_tokens: 60,
      output_tokens_details: { reasoning_tokens: 20 },
      total_tokens: 2160,
    },
  };
}

function send(response: ServerResponse, status: number, body: unknown): void {
  response
    .writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
    .end(JSON.stringify(body));
}

/** Atende `/openai/…` (o app aponta `WARDEN_API_BASE_OPENAI` para cá). */
export function serveOpenai(
  request: IncomingMessage,
  response: ServerResponse,
  path: string,
): void {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    if (
      path.includes(OPENAI_E2E_KEY) ||
      request.headers.authorization !== `Bearer ${OPENAI_E2E_KEY}`
    ) {
      state.refusals.push('chave');
      send(response, 401, {
        error: {
          message: 'Incorrect API key provided.',
          type: 'invalid_request_error',
          param: null,
          code: 'invalid_api_key',
        },
      });
      return;
    }
    const route = path.split('?')[0] ?? '';
    if (request.method === 'GET' && route.endsWith('/v1/models')) {
      send(response, 200, {
        object: 'list',
        data: ['gpt-9-astra', 'gpt-9-luna', 'gpt-9-sol', 'gpt-9.1-sol'].map((id) => ({
          id,
          object: 'model',
          owned_by: 'system',
        })),
      });
      return;
    }
    if (!route.endsWith('/v1/responses')) {
      send(response, 404, { error: { message: 'Not found', type: 'invalid_request_error' } });
      return;
    }
    const body = Buffer.concat(chunks).toString('utf8');
    const parsed = JSON.parse(body) as Json;
    const refused = validate(parsed);
    if (refused) {
      state.refusals.push(refused);
      send(response, 400, {
        error: { message: refused, type: 'invalid_request_error', param: null, code: null },
      });
      return;
    }
    state.generate.push(body);
    const answer = reply(Array.isArray(parsed.input) ? (parsed.input as Json[]) : []);
    emitted.push(answer.output as Json[]);
    send(response, 200, answer);
  });
}
