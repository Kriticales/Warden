/**
 * Gemini simulado dos E2E (D-04): `models.list`, `countTokens` e `generateContent`, com as regras
 * da API real que importam para o laço (as mesmas do servidor simulado da `warden-ai`):
 *
 * - a chave vem só no cabeçalho `x-goog-api-key` (nunca `key=` na URL);
 * - no turno atual, cada `content` do modelo tem de ser **idêntico** ao que o simulado mandou
 *   (assinaturas intactas) e a primeira `functionCall` de cada passo traz a `thoughtSignature`;
 * - depois de N chamadas vem uma mensagem do usuário com as N respostas e os mesmos `id`.
 *
 * Roteiro fixo: a cada pergunta nova, a IA pede a visão geral, a lista de mods e uma config; com
 * as respostas, responde pela `responder` com uma afirmação que cita literalmente a primeira
 * linha da visão geral (vira "verificada") e outra com um id que nenhuma ferramenta devolveu
 * (vira "não verificado"). Os corpos recebidos ficam guardados para o teste comparar com o que
 * a tela mostra (`GET /__gemini`).
 */
import type { IncomingMessage, ServerResponse } from 'node:http';

/** A chave que os E2E gravam no cofre de teste. */
export const GEMINI_E2E_KEY = 'chave-e2e-do-gemini-simulado';

const MODEL_LITE = 'gemini-9.1-flash-lite';

interface GeminiState {
  /** Corpos de `generateContent` aceitos, como chegaram. */
  generate: string[];
  /** Corpos de `countTokens`. */
  countTokens: string[];
  /** Recusas (histórico alterado, chave errada). */
  refusals: string[];
}

let state: GeminiState = { generate: [], countTokens: [], refusals: [] };
let emitted: unknown[] = [];
let counter = 0;

export function geminiState(): GeminiState {
  return state;
}

export function resetGemini(): void {
  state = { generate: [], countTokens: [], refusals: [] };
  emitted = [];
}

type Json = Record<string, unknown>;

function parts(content: unknown): Json[] {
  const list = (content as { parts?: unknown } | null)?.parts;
  return Array.isArray(list) ? (list as Json[]) : [];
}

function role(content: unknown): string {
  const value = (content as { role?: unknown } | null)?.role;
  return typeof value === 'string' ? value : '';
}

/** As regras de histórico da API real (thought-signatures do Gemini 3). */
function validate(contents: unknown[]): string | null {
  let start = -1;
  contents.forEach((content, index) => {
    if (role(content) === 'user' && parts(content).some((part) => 'text' in part)) start = index;
  });
  if (start < 0) return 'nenhuma mensagem do usuário';
  for (let i = start + 1; i < contents.length; i++) {
    const content = contents[i];
    if (role(content) !== 'model') continue;
    const calls = parts(content).filter((part) => 'functionCall' in part);
    if (calls.length > 0 && !('thoughtSignature' in (calls[0] ?? {}))) {
      return `Function call in the ${String(i)}. content block is missing a thought_signature.`;
    }
    if (!emitted.some((sent) => JSON.stringify(sent) === JSON.stringify(content))) {
      return `Corrupted thought signature or modified model content in the ${String(i)}. content block.`;
    }
    if (calls.length > 0) {
      const next = contents[i + 1];
      const responses = parts(next).filter((part) => 'functionResponse' in part);
      if (role(next) !== 'user' || responses.length !== calls.length) {
        return `Function calls in the ${String(i)}. content block have no responses.`;
      }
    }
  }
  return null;
}

function calls(list: [string, Json][]): Json {
  const callParts = list.map(([name, args], index) => {
    counter += 1;
    const part: Json = { functionCall: { name, args, id: `call_${String(counter)}` } };
    if (index === 0) part.thoughtSignature = `c2lnLWUyZS0${String(counter)}`;
    return part;
  });
  return {
    candidates: [{ content: { role: 'model', parts: callParts }, finishReason: 'STOP', index: 0 }],
    usageMetadata: { promptTokenCount: 2100, candidatesTokenCount: 40 },
    modelVersion: MODEL_LITE,
  };
}

/** O primeiro `texto` devolvido pela visão geral nesta rodada. */
function overviewText(contents: unknown[]): string {
  const last = contents.at(-1);
  for (const part of parts(last)) {
    const response = (part.functionResponse as Json | undefined)?.response as Json | undefined;
    const items = response?.itens as { id: string; texto: string }[] | undefined;
    const overview = items?.find((item) => item.id === 'pack:overview');
    if (overview) return overview.texto.split('\n')[0] ?? '';
  }
  return '';
}

function reply(contents: unknown[]): Json {
  const last = contents.at(-1);
  const isQuestion = role(last) === 'user' && parts(last).some((part) => 'text' in part);
  if (isQuestion) {
    return calls([
      ['get_pack_overview', {}],
      ['list_mods', {}],
      ['read_config', { path: 'config/exemplo.toml' }],
    ]);
  }
  const quote = overviewText(contents);
  return calls([
    [
      'responder',
      {
        resumo: 'O pack ainda não foi testado; não há travamento para analisar.',
        semConclusao: false,
        propostas: [],
        achados: [
          {
            afirmacao: 'A visão geral do pack foi lida pela IA.',
            mods: [],
            confianca: 'alta',
            evidencias: [{ id: 'pack:overview', citacao: quote }],
          },
          {
            afirmacao: 'O pack travou por falta de memória.',
            mods: [],
            confianca: 'alta',
            evidencias: [{ id: 'crash:inexistente', citacao: 'OutOfMemoryError' }],
          },
        ],
      },
    ],
  ]);
}

function send(response: ServerResponse, status: number, body: unknown): void {
  response
    .writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
    .end(JSON.stringify(body));
}

/** Atende `/gemini/…` (o app aponta `WARDEN_API_BASE_GEMINI` para cá). */
export function serveGemini(
  request: IncomingMessage,
  response: ServerResponse,
  path: string,
): void {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    const key = request.headers['x-goog-api-key'];
    if (path.includes('key=') || key !== GEMINI_E2E_KEY) {
      state.refusals.push('chave');
      send(response, 400, {
        error: { code: 400, status: 'INVALID_ARGUMENT', details: [{ reason: 'API_KEY_INVALID' }] },
      });
      return;
    }
    const route = path.split('?')[0] ?? '';
    if (request.method === 'GET' && route.endsWith('/v1beta/models')) {
      send(response, 200, {
        models: [
          {
            name: `models/${MODEL_LITE}`,
            supportedGenerationMethods: ['generateContent', 'countTokens'],
          },
          {
            name: 'models/gemini-9.1-flash',
            supportedGenerationMethods: ['generateContent', 'countTokens'],
          },
          {
            name: 'models/gemini-9.2-flash-preview',
            supportedGenerationMethods: ['generateContent'],
          },
        ],
      });
      return;
    }
    const body = Buffer.concat(chunks).toString('utf8');
    if (route.endsWith(':countTokens')) {
      state.countTokens.push(body);
      send(response, 200, { totalTokens: Math.ceil(body.length / 4) });
      return;
    }
    if (!route.endsWith(':generateContent')) {
      send(response, 404, { error: { code: 404 } });
      return;
    }
    const parsed = JSON.parse(body) as { contents?: unknown[] };
    const contents = parsed.contents ?? [];
    const refused = validate(contents);
    if (refused) {
      state.refusals.push(refused);
      send(response, 400, { error: { code: 400, status: 'INVALID_ARGUMENT', message: refused } });
      return;
    }
    state.generate.push(body);
    const answer = reply(contents);
    const candidates = answer.candidates as { content: unknown }[];
    emitted.push(candidates[0]?.content);
    send(response, 200, answer);
  });
}
