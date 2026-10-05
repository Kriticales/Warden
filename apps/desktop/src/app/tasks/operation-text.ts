/**
 * Textos de uma operação na gaveta de Tarefas e nos toasts: nome do tipo, etapa, progresso e
 * quando terminou. Tudo do catálogo pt-BR (`tarefas.ts` e os catálogos das áreas).
 */
import i18n, { resources } from '../../i18n';
import { dayRelation, formatBytes, formatDate, formatInteger, formatTime } from '../../lib/format';
import type { OperationSnapshot, Progress } from '../../lib/ipc/bindings';

type Namespace = keyof (typeof resources)['pt-BR'];

const NAMESPACES = Object.keys(resources['pt-BR']) as Namespace[];

/**
 * Tradução de uma chave montada em tempo de execução (tipo de operação, etapa), que o tipo do
 * catálogo não conhece. Quem chama confere antes com `i18n.exists`.
 */
const translateDynamic = i18n.t.bind(i18n) as (
  key: string,
  options: { ns: Namespace | readonly Namespace[] },
) => string;

/** Nome do tipo (`tipos.<kind>` em `tarefas.ts`), ou "Tarefa do Warden". */
export function operationName(kind: string): string {
  const key = `tipos.${kind}`;
  if (i18n.exists(key, { ns: 'tarefas' })) {
    return translateDynamic(key, { ns: 'tarefas' });
  }
  return i18n.t('tipoDesconhecido', { ns: 'tarefas' });
}

/** Texto da etapa (`labelKey` de qualquer catálogo, `área:chave` ou só a chave), ou `null`. */
export function stageLabel(operation: OperationSnapshot): string | null {
  const key = operation.stage?.labelKey;
  if (!key) {
    return null;
  }
  if (!i18n.exists(key, { ns: NAMESPACES })) {
    return null;
  }
  return translateDynamic(key, { ns: NAMESPACES });
}

function amount(value: number, unit: Progress['unit']): string {
  return unit === 'bytes' ? formatBytes(value) : formatInteger(value);
}

/** "160 de 420 MB", "12 de 128" ou só a quantidade feita. */
export function progressText(progress: Progress): string {
  const feito = amount(progress.current, progress.unit);
  if (progress.total === null) {
    return i18n.t('progresso.semTotal', { ns: 'tarefas', feito });
  }
  return i18n.t('progresso.de', {
    ns: 'tarefas',
    feito,
    total: amount(progress.total, progress.unit),
  });
}

/** Porcentagem (0 a 100) ou `null` quando o total é desconhecido. */
export function progressPercent(progress: Progress | null): number | null {
  const total = progress?.total;
  if (!progress || total === null || total === undefined || total <= 0) {
    return null;
  }
  return Math.min(100, (progress.current / total) * 100);
}

/** "14:01", "ontem, 21:40" ou "28/09/2026, 10:12". */
export function finishedWhen(ms: number, now: number = Date.now()): string {
  const hora = formatTime(ms);
  switch (dayRelation(ms, now)) {
    case 'today':
      return hora;
    case 'yesterday':
      return i18n.t('quando.ontem', { ns: 'tarefas', hora });
    case 'other':
      return i18n.t('quando.data', { ns: 'tarefas', data: formatDate(ms), hora });
  }
}
