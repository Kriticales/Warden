/**
 * Números da memória do jogo no formato do pt-BR (L-11), além dos da faixa (`../format.ts`) e
 * da memória como o usuário escolhe (`formatMemory`, "6 GB"): o tempo parado em coletas
 * ("1,2 s", "2 min 5 s"), a quantidade de coletas ("1.902"), o rótulo do gráfico e as frases
 * dos achados de memória a partir dos números que a sessão grava (`maxMb`, `seconds`…).
 */
import i18n from '../../../../i18n';
import { formatDuration, formatMemory } from '../../text';

const t = i18n.getFixedT(null, 'memoria');

const COUNT_FORMAT = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 0 });
const SECONDS_FORMAT = new Intl.NumberFormat('pt-BR', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});

/** Quantas coletas: "214", "1.902". */
export function countText(count: number): string {
  return COUNT_FORMAT.format(count);
}

/** Tempo parado em coletas: "0,4 s", "18,4 s"; a partir de 1 minuto, "2 min 5 s". */
export function gcTimeText(ms: number): string {
  if (ms < 60_000) {
    return t('segundos', { valor: SECONDS_FORMAT.format(ms / 1000) });
  }
  return formatDuration(ms);
}

/** Os rótulos do gráfico dos últimos 5 minutos. */
export function seriesLabel(seriesMb: readonly number[]): string {
  if (seriesMb.length === 0) return '';
  const low = Math.min(...seriesMb);
  const high = Math.max(...seriesMb);
  if (formatMemory(low) === formatMemory(high)) {
    return t('faixa.graficoIgual', { valor: formatMemory(high) });
  }
  return t('faixa.grafico', { menor: formatMemory(low), maior: formatMemory(high) });
}

function number(value: string | undefined): number | null {
  if (value === undefined) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

/**
 * As frases dos achados e das evidências de memória com os números da sessão já formatados:
 * `maxMb` → `memoria` ("6 GB"), `suggestedMb` → `sugerida`, `seconds` → `duracao`
 * ("1 min 12 s"), `percent` → `percentual` ("93%"), `gcMs` → `parado` ("8,0 s") e `peakMb` →
 * `pico`. Só os que existem.
 */
export function memoryPhraseParams(
  values: Partial<Record<string, string>>,
): Record<string, string> {
  const out: Record<string, string> = {};
  const maxMb = number(values.maxMb);
  const suggested = number(values.suggestedMb);
  const seconds = number(values.seconds);
  const percent = number(values.percent);
  const gcMs = number(values.gcMs);
  const peak = number(values.peakMb);
  if (maxMb !== null) out.memoria = formatMemory(maxMb);
  if (suggested !== null) out.sugerida = formatMemory(suggested);
  if (seconds !== null) out.duracao = formatDuration(seconds * 1000);
  if (percent !== null) out.percentual = `${COUNT_FORMAT.format(percent)}%`;
  if (gcMs !== null) out.parado = gcTimeText(gcMs);
  if (peak !== null) out.pico = formatMemory(peak);
  return out;
}
