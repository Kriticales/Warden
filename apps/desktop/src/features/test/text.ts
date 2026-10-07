/**
 * Textos do Testar que dependem de valores: durações ("1 min 42 s"), memória ("6 GB"), quando
 * foi o teste ("hoje, 14:20") e as linhas do Warden no console (`origin: warden`, com a chave
 * do catálogo `teste.console.*` e os valores crus do backend). Funções puras sobre o i18n.
 */
import i18n, { resources } from '../../i18n';
import { dayRelation, formatDate, formatInteger, formatTime } from '../../lib/format';
import type { ConsoleLine, Outcome } from '../../lib/ipc/bindings';

const t = i18n.getFixedT(null, 'teste');

/**
 * Tradução de uma chave montada em tempo de execução (mensagens do Warden no console, etapas),
 * que o tipo do catálogo não conhece. Quem chama confere antes que a chave existe.
 */
const translateDynamic = i18n.t.bind(i18n) as (
  key: string,
  options: Record<string, unknown>,
) => string;

/** Duração legível: "48 s", "1 min 42 s", "12 min", "1 h 5 min". */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) {
    return m > 0 ? t('duracao.horasMinutos', { h, m }) : t('duracao.horas', { h });
  }
  if (m > 0) {
    // A partir de 10 minutos, os segundos só atrapalham.
    return s > 0 && m < 10 ? t('duracao.minutosSegundos', { m, s }) : t('duracao.minutos', { m });
  }
  return t('duracao.segundos', { s });
}

/** Memória em MB como o usuário escolhe: "6 GB", "1,5 GB", "512 MB". */
export function formatMemory(mb: number): string {
  if (mb < 1024) {
    return `${formatInteger(mb)} MB`;
  }
  const gb = mb / 1024;
  const text = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 }).format(gb);
  return t('esteTeste.memoriaGb', { gb: text });
}

/** "hoje, 14:20", "ontem, 21:40" ou "28/09/2026, 10:12". */
export function formatWhen(ms: number, now: number = Date.now()): string {
  const hora = formatTime(ms);
  switch (dayRelation(ms, now)) {
    case 'today':
      return t('quando.hoje', { hora });
    case 'yesterday':
      return t('quando.ontem', { hora });
    case 'other':
      return t('quando.data', { data: formatDate(ms), hora });
  }
}

/** Resultado em poucas palavras ("travou"), para o menu e a lista de sessões. */
export function outcomeShort(outcome: Outcome): string {
  return t(`resultadoCurto.${outcome}`);
}

/** Mensagens do Warden no console que o catálogo conhece. */
const WARDEN_MESSAGES = new Set([
  'console.abrindo',
  'console.fechou',
  'console.encerrado',
  'console.travou',
]);

/**
 * O texto de uma linha do console: a do jogo como saiu (sem tradução, SPEC T13); a do Warden
 * pela chave do catálogo, com durações e memória formatadas.
 */
export function consoleText(line: ConsoleLine): string {
  if (line.origin === 'game') {
    return line.text;
  }
  if (!WARDEN_MESSAGES.has(line.text)) {
    return line.text;
  }
  const { duracaoMs, memoriaMb, ...rest } = line.params;
  const values: Record<string, string> = { ...rest };
  if (duracaoMs !== undefined) values.duracao = formatDuration(Number(duracaoMs));
  if (memoriaMb !== undefined) values.memoria = formatMemory(Number(memoriaMb));
  return translateDynamic(line.text, { ns: 'teste', ...values });
}

/** Hora de uma linha do console: a do relógio do Warden ("14:20:05") ou a escrita pelo jogo. */
export function consoleTime(line: ConsoleLine): string {
  if (line.atMs !== null) {
    return clock.format(line.atMs);
  }
  return line.time ?? '';
}

const clock = new Intl.DateTimeFormat('pt-BR', {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
});

/** A linha inteira como texto, para copiar e salvar: "14:20:05 [INFO] [logger] mensagem". */
export function consoleLineAsText(line: ConsoleLine): string {
  const level = line.origin === 'warden' ? 'WARDEN' : (line.level?.toUpperCase() ?? '');
  const source = line.origin === 'warden' ? t('console.origemWarden') : (line.logger ?? '');
  const parts = [consoleTime(line), level ? `[${level}]` : '', source ? `[${source}]` : ''];
  return `${parts.filter(Boolean).join(' ')} ${consoleText(line)}`.trim();
}

type Namespace = keyof (typeof resources)['pt-BR'];

const NAMESPACES = Object.keys(resources['pt-BR']) as Namespace[];

/** O texto de uma etapa do backend (`labelKey` de qualquer catálogo), ou `null`. */
export function stageText(labelKey: string): string | null {
  if (!i18n.exists(labelKey, { ns: NAMESPACES })) {
    return null;
  }
  return translateDynamic(labelKey, { ns: NAMESPACES });
}
