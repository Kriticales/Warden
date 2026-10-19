/**
 * O roteiro da IA simulada dos E2E, igual para a OpenAI (`openai.ts`) e o Claude
 * (`anthropic.ts`), no mesmo desenho do Gemini simulado (`gemini.ts`, D-04 e D-14), para os E2E
 * por provedor (D-18) verem o mesmo resultado em qualquer um:
 *
 * - a cada pergunta nova, a IA pede a visão geral, a lista de mods e uma config; com as
 *   respostas, responde pela `responder` com uma afirmação que cita literalmente a primeira linha
 *   da visão geral (vira "verificada") e outra com um id que nenhuma ferramenta devolveu (vira
 *   "não verificado");
 * - com [`PROPOSALS_MARKER`] no texto da pessoa, o roteiro das propostas: abre o Sodium, lê a
 *   config e procura issues; propõe duas mudanças na config (com evidência) e uma remoção com
 *   evidência inventada (o Warden recusa); por fim responde pela `responder`.
 */

/** Uma chamada de ferramenta: nome e argumentos. */
export type Call = [string, Record<string, unknown>];

/** No texto da pessoa, liga o roteiro das propostas (o mesmo marcador do Gemini simulado). */
export const PROPOSALS_MARKER = '[e2e-propostas]';

function proposeConfig(value: string): Call {
  return [
    'propose_edit_config',
    {
      path: 'config/exemplo.toml',
      key: ['geral', 'segredoDaConfig'],
      value,
      motivo: `Com ${value}, o valor fica dentro do que o mod aceita.`,
      evidencias: [{ id: 'config:config/exemplo.toml#L2', citacao: 'segredoDaConfig = 42' }],
    },
  ];
}

function proposals(step: number): Call[] {
  if (step === 0) {
    return [
      ['get_mod_details', { mod_id: 'sodium' }],
      ['read_config', { path: 'config/exemplo.toml' }],
      ['search_mod_issues', { mod_id: 'sodium', query: 'NullPointerException EntityRenderer' }],
    ];
  }
  if (step === 1) {
    return [
      proposeConfig('43'),
      proposeConfig('44'),
      [
        'propose_remove_mod',
        {
          mod_id: 'sodium',
          motivo: 'Inventado.',
          evidencias: [{ id: 'crash:inexistente', citacao: 'Sodium quebrou' }],
        },
      ],
    ];
  }
  return [
    [
      'responder',
      {
        resumo: 'O valor da config passa do que o mod aceita; mudar a chave deve resolver.',
        semConclusao: false,
        propostas: [],
        achados: [
          {
            afirmacao: 'A config do pack tem o valor 42.',
            mods: [],
            confianca: 'alta',
            evidencias: [{ id: 'config:config/exemplo.toml#L2', citacao: 'segredoDaConfig = 42' }],
          },
        ],
      },
    ],
  ];
}

/** A primeira linha do `texto` da visão geral entre os resultados da última rodada. */
function overviewLine(results: unknown[]): string {
  for (const result of results) {
    const items = (result as { itens?: { id: string; texto: string }[] } | null)?.itens;
    const overview = items?.find((item) => item.id === 'pack:overview');
    if (overview) return overview.texto.split('\n')[0] ?? '';
  }
  return '';
}

/**
 * As chamadas da próxima resposta: `question` é o texto da última pergunta, `step` quantas
 * respostas do modelo já vieram depois dela e `results` o conteúdo (JSON) dos resultados da
 * última rodada.
 */
export function nextCalls(question: string, step: number, results: unknown[]): Call[] {
  if (question.includes(PROPOSALS_MARKER)) return proposals(step);
  if (step === 0) {
    return [
      ['get_pack_overview', {}],
      ['list_mods', {}],
      ['read_config', { path: 'config/exemplo.toml' }],
    ];
  }
  return [
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
            evidencias: [{ id: 'pack:overview', citacao: overviewLine(results) }],
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
  ];
}

/** JSON com as chaves em ordem: a comparação vale pelo conteúdo, não pela ordem das chaves. */
export function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) =>
      a < b ? -1 : a > b ? 1 : 0,
    );
    return `{${entries.map(([key, inner]) => `${JSON.stringify(key)}:${canonical(inner)}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

/** O conteúdo de um resultado de ferramenta (texto JSON), ou `null`. */
export function parseResult(text: unknown): unknown {
  if (typeof text !== 'string') return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}
