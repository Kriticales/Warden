/**
 * Textos derivados das conversas (SPEC T14 ✦; glossário da QUALITY §8): o título de cada bloco
 * "Enviado à IA" (a partir da ferramenta e dos argumentos que a IA pediu), o rótulo de cada selo
 * de evidência (de onde veio o trecho), o medidor de confiança e o erro gravado na conversa
 * como `AppError`, para a frase traduzida de sempre.
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

/** Id de sessão das ferramentas de log para o arquivo escolhido no computador. */
export const LOCAL_FILE_SESSION = 'arquivo';

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
      return tt('enviado.buscaLog', { busca: text(args.query), sessao });
    case 'read_log':
      return tt('enviado.trechoLog', {
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
    default:
      return tt('enviado.outra', { nome: step.name });
  }
}

/** Tamanho do que foi enviado ("2,1 KB"). */
export function sentSize(sent: string): string {
  return formatBytes(new TextEncoder().encode(sent).length);
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
    default:
      return id;
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
