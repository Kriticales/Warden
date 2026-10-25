/**
 * Textos derivados das conversas do Sensor Sculk (SPEC T14; glossário da QUALITY §8): o título
 * de cada consulta "Enviado ao Sculk" (a partir da ferramenta e dos argumentos que o Sculk
 * pediu), o rótulo de cada selo de evidência (de onde veio o trecho), o medidor de confiança e o
 * erro gravado na conversa como `AppError`, para a frase traduzida de sempre.
 */
import i18n from '../../../i18n';
import type {
  AppError,
  ConfidenceMeter,
  ConversationOrigin,
  ErrorCode,
  AiSymptom,
  ToolStep,
  TurnFailure,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { d14EvidenceLabel, d14StepLabel } from './proposals/text';

/** Id de sessão das ferramentas de log para o arquivo escolhido no computador. */
export const LOCAL_FILE_SESSION = 'arquivo';
/** Id de sessão das ferramentas de log para o log aberto por um link (conversa sem pack). */
export const LINK_SESSION = 'link';

const tt = (key: string, options?: Record<string, unknown>): string =>
  (i18n.t as unknown as (key: string, options?: Record<string, unknown>) => string)(
    `ia:${key}`,
    options,
  );

const numero = new Intl.NumberFormat('pt-BR');

/** `2026-10-03_21-10-00` → "03/10 às 21:10" (o nome da pasta da sessão do Testar). */
export function sessionLabel(id: string): string {
  if (id === LOCAL_FILE_SESSION) {
    return tt('origem.arquivoCurto');
  }
  const match = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})/.exec(id);
  if (!match) {
    return id;
  }
  const [, , month, day, hour, minute] = match;
  return tt('origem.sessaoCurta', {
    dia: `${day ?? ''}/${month ?? ''}`,
    hora: `${hour ?? ''}:${minute ?? ''}`,
  });
}

function parseArgs(step: ToolStep): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(step.args);
    return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function text(value: unknown): string {
  return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}

/** O que foi enviado, em palavras: "trecho do log do teste de 03/10 às 21:10, linhas 1.200–1.260". */
export function stepLabel(step: ToolStep): string {
  const args = parseArgs(step);
  const sessao = sessionLabel(text(args.session_id));
  const mod = text(args.mod_id);
  switch (step.name) {
    case 'get_pack_overview':
      return tt('enviado.visaoGeral');
    case 'list_mods':
      return text(args.filter)
        ? tt('enviado.listaFiltrada', { filtro: text(args.filter) })
        : tt('enviado.lista');
    case 'get_mod_details':
      return tt('enviado.detalhes', { mod });
    case 'get_findings':
      return tt('enviado.achados');
    case 'list_sessions':
      return tt('enviado.testes');
    case 'get_session':
      return tt('enviado.teste', { sessao });
    case 'get_crash_report':
      return tt('enviado.crashReport', { sessao });
    case 'search_log':
      return args.session_id === LINK_SESSION
        ? tt('enviado.buscaLink', { busca: text(args.query) })
        : tt('enviado.buscaLog', { busca: text(args.query), sessao });
    case 'read_log':
      return tt(args.session_id === LINK_SESSION ? 'enviado.trechoLink' : 'enviado.trechoLog', {
        sessao,
        de: numero.format(Number(args.from) || 0),
        ate: numero.format(Number(args.to) || 0),
      });
    case 'search_configs':
      return tt('enviado.buscaConfigs', { busca: text(args.query) });
    case 'read_config':
      return tt('enviado.config', { arquivo: text(args.path) });
    case 'get_history':
      return tt('enviado.historico');
    case 'diff_versions':
      return tt('enviado.diferenca', { de: text(args.from), para: text(args.to) });
    case 'get_dependents':
      return tt('enviado.dependentes', { mod });
    case 'why_in_pack':
      return tt('enviado.porQue', { mod });
    case 'get_test_performance':
      return tt('enviado.desempenho');
    case 'get_performance_profile':
      return tt('enviado.perfilDesempenho', { sessao });
    default:
      // D-14: consultas externas, raio-x, busca do culpado e propostas.
      return d14StepLabel(step, args) ?? tt('enviado.outra', { nome: step.name });
  }
}

/** Bytes do texto em UTF-8 (o que foi enviado). */
export function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** Tamanho do que foi enviado ("2,1 KB"). */
export function sentSize(sent: string): string {
  return formatBytes(byteLength(sent));
}

/** De onde veio o trecho, pelo id da evidência ("log, linha 1.234"). */
export function evidenceLabel(id: string): string {
  const line = /#L(\d+)$/.exec(id);
  const linha = line ? numero.format(Number(line[1])) : null;
  const [kind = '', ...rest] = id.split(':');
  const value = rest.join(':').replace(/#L\d+$/, '');
  switch (kind) {
    case 'log':
      return linha ? tt('selo.log', { linha }) : tt('selo.logSemLinha');
    case 'crash':
      return linha ? tt('selo.crash', { linha }) : tt('selo.crashSemLinha');
    case 'config':
      return linha ? tt('selo.config', { arquivo: value, linha }) : value;
    case 'mod':
      return tt('selo.mod', { mod: value });
    case 'jar':
      return tt('selo.jar', { mod: value });
    case 'finding':
      return tt('selo.achado');
    case 'session':
      return tt('selo.teste', { sessao: sessionLabel(value.split('#')[0] ?? value) });
    case 'version':
      return tt('selo.versao', { versao: value });
    case 'graph':
      return tt('selo.grafo', { mod: value });
    case 'pack':
      return tt('selo.visaoGeral');
    case 'perf':
      return value === 'comparacao'
        ? tt('selo.comparacao')
        : tt('selo.desempenho', { sessao: sessionLabel(value) });
    case 'perfprofile':
      return tt('selo.perfilDesempenho', { sessao: sessionLabel(value.split('#')[0] ?? value) });
    default:
      // D-14: issues, changelogs, raio-x e busca do culpado.
      return d14EvidenceLabel(kind, value) ?? id;
  }
}

/** Quantos blocos acesos no medidor. */
export function meterBlocks(confidence: ConfidenceMeter): number {
  return { low: 1, medium: 2, high: 3 }[confidence];
}

/** O nome do sintoma. */
export function symptomLabel(symptom: AiSymptom): string {
  return tt(`sintoma.${symptom}.titulo`);
}

/** "Conversa sobre …": o que foi analisado, no meio da frase. */
export function aboutLabel(origin: ConversationOrigin): string {
  switch (origin.kind) {
    case 'session':
      return origin.lastCrash
        ? tt('origem.sobre.travamento', { sessao: sessionLabel(origin.sessionId) })
        : tt('origem.sobre.teste', { sessao: sessionLabel(origin.sessionId) });
    case 'localFile':
      return tt('origem.sobre.arquivo', { arquivo: origin.fileName });
    case 'packOnly':
      return tt('origem.sobre.soPack');
    case 'pasteLink':
      return tt('origem.sobre.link', { site: origin.site === 'mclogs' ? 'mclo.gs' : 'pastebin' });
  }
}

/** O que foi analisado ("Travamento de 03/10 às 21:10", "crash-x.txt", "Só o pack"). */
export function originLabel(origin: ConversationOrigin): string {
  switch (origin.kind) {
    case 'session':
      return origin.lastCrash
        ? tt('origem.travamento', { sessao: sessionLabel(origin.sessionId) })
        : tt('origem.teste', { sessao: sessionLabel(origin.sessionId) });
    case 'localFile':
      return origin.fileName;
    case 'packOnly':
      return tt('origem.soPack');
    case 'pasteLink':
      return tt('origem.link', { site: origin.site === 'mclogs' ? 'mclo.gs' : 'pastebin' });
  }
}

/** O erro gravado na conversa, no formato do `AppError` (mesma frase dos outros erros). */
export function failureError(failure: TurnFailure): AppError {
  return {
    code: { domain: failure.domain, code: failure.code } as ErrorCode,
    params: failure.params,
    detail: failure.detail,
    retryable: failure.retryable,
    operationId: null,
  };
}

/** Tokens em "~182 mil". */
export function tokensLabel(tokens: number): string {
  if (tokens >= 1000) {
    return tt('tokens.mil', { valor: numero.format(Math.round(tokens / 1000)) });
  }
  return tt('tokens.unidades', { valor: numero.format(tokens) });
}
