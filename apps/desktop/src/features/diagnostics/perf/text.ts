/**
 * Textos do bloco "Desempenho do último teste": a memória "5,8 de 6 GB", a variação do tempo
 * para abrir ("+24% que na 0.1.1") e o aviso de versão mais pesada da T33, com os números.
 */
import i18n from '../../../i18n';
import type { LastTestPerf, PreviousLoad, VersionComparison } from '../../../lib/ipc/bindings';
import { loadTimeText, memoryText } from '../../test/perf-strip/format';

const t = i18n.getFixedT(null, 'desempenho');

/** Diferença abaixo da qual os tempos contam como iguais (a mesma da faixa de desempenho). */
const SAME_TIME_MS = 1000;

const GB = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 });
const MS = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 });

/** Um total de memória como o usuário escolheu: "6 GB", "4 GB", "512 MB". */
function totalText(mb: number): string {
  return mb >= 1024 ? `${GB.format(mb / 1024)} ${t('unidade.gb')}` : memoryText(mb);
}

/** "5,8 de 6 GB"; abaixo de 1 GB, cada lado com a sua unidade ("900 MB de 2 GB"). */
export function memoryOfText(usedMb: number, maxMb: number): string {
  if (usedMb >= 1024 && maxMb >= 1024) {
    return t('ultimoTeste.memoriaDe', {
      usado: GB.format(usedMb / 1024),
      maximo: totalText(maxMb),
    });
  }
  return t('ultimoTeste.memoriaDe', { usado: memoryText(usedMb), maximo: totalText(maxMb) });
}

/** Um tempo por tick: "38 ms", "12,5 ms". */
export function tickText(ms: number): string {
  return t('ultimoTeste.tickValor', { ms: MS.format(ms) });
}

/** A variação do tempo para abrir em %: positiva quando abriu mais devagar. */
export function loadDelta(loadedMs: number, previous: PreviousLoad): number | null {
  if (Math.abs(loadedMs - previous.loadedMs) < SAME_TIME_MS || previous.loadedMs <= 0) {
    return null;
  }
  return Math.round(((loadedMs - previous.loadedMs) / previous.loadedMs) * 100);
}

/** "+24% que na 0.1.1", "−8% que na 0.1.1" ou "igual à 0.1.1". */
export function loadDeltaText(loadedMs: number, previous: PreviousLoad): string {
  const versao = previous.packVersion;
  const delta = loadDelta(loadedMs, previous);
  if (delta === null || delta === 0) {
    return t('ultimoTeste.variacaoIgual', { versao });
  }
  return t('ultimoTeste.variacao', {
    sinal: delta > 0 ? '+' : '−',
    percentual: Math.abs(delta),
    versao,
  });
}

function metricValue(comparison: VersionComparison, value: number): string {
  switch (comparison.metric) {
    case 'memory':
      return memoryText(value);
    case 'tick':
      return tickText(value);
    case 'loadTime':
    case null:
      return loadTimeText(value);
  }
}

/** O aviso de versão mais pesada (`null` quando a comparação não passou do critério). */
export function heavierText(comparison: VersionComparison): string | null {
  if (
    comparison.status !== 'heavier' ||
    comparison.metric === null ||
    comparison.current === null ||
    comparison.previous === null
  ) {
    return null;
  }
  return t(`ultimoTeste.maisPesada.${comparison.metric}`, {
    versao: comparison.version,
    anterior: comparison.previousVersion,
    percentual: comparison.percent ?? 0,
    agora: metricValue(comparison, comparison.current),
    antes: metricValue(comparison, comparison.previous),
  });
}

/** Ficou mais lento: passou do critério da T33 ou abriu mais devagar que a última versão. */
export function gotSlower(perf: LastTestPerf): boolean {
  if (perf.comparison?.status === 'heavier') return true;
  if (!perf.previous) return false;
  const delta = loadDelta(perf.loadedMs, perf.previous);
  return delta !== null && delta > 0;
}

/** O assunto da conversa nova com o Sculk ("por que ficou mais lento"). */
export function sculkSubject(perf: LastTestPerf): string {
  const aviso = perf.comparison ? heavierText(perf.comparison) : null;
  if (aviso) {
    return t('ultimoTeste.assunto', { aviso });
  }
  const abrir = loadTimeText(perf.loadedMs);
  const variacao = perf.previous ? ` (${loadDeltaText(perf.loadedMs, perf.previous)})` : '';
  return t('ultimoTeste.assuntoSemAviso', { abrir: `${abrir}${variacao}` });
}
