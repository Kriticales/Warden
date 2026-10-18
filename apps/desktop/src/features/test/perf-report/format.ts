/**
 * Números da parte "Desempenho" no formato do pt-BR: o tempo de carregamento de um mod ("288 ms",
 * "1,4 s", "2 min 5 s"), o tempo por tick ("1,6 ms") e as porcentagens ("31%", "0,5%").
 */
import { formatDuration } from '../text';

const ONE_DECIMAL = new Intl.NumberFormat('pt-BR', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
const UP_TO_ONE_DECIMAL = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 });
const INTEGER = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 0 });

/** Tempo de carregamento: abaixo de 1 s em ms, até 1 minuto com uma casa, depois em minutos. */
export function loadMsText(ms: number): string {
  if (ms < 1000) {
    return `${INTEGER.format(ms)} ms`;
  }
  if (ms < 60_000) {
    return `${ONE_DECIMAL.format(ms / 1000)} s`;
  }
  return formatDuration(ms);
}

/** Tempo por tick: "1,6 ms", "213,7 ms". */
export function msptText(ms: number): string {
  return `${UP_TO_ONE_DECIMAL.format(ms)} ms`;
}

/** Ticks por segundo: "20", "16,7". */
export function tpsText(tps: number): string {
  return UP_TO_ONE_DECIMAL.format(tps);
}

/** Porcentagem já calculada (com uma casa): "31%", "0,5%". */
export function percentText(percent: number): string {
  return `${UP_TO_ONE_DECIMAL.format(percent)}%`;
}
