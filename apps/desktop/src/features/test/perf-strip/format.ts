/**
 * Números da faixa de desempenho no formato do pt-BR: memória com vírgula decimal e unidade
 * ("4,2 GB", "512 MB"), tempos ("1 min 42 s") e a comparação com a última versão testada
 * ("18 s a mais que na 1.4.2").
 */
import i18n from '../../../i18n';
import type { PreviousLoad } from '../../../lib/ipc/bindings';
import { formatDuration } from '../text';

const t = i18n.getFixedT(null, 'desempenho');

/** Diferença abaixo da qual os tempos contam como iguais. */
const SAME_TIME_MS = 1000;

const GB_FORMAT = new Intl.NumberFormat('pt-BR', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
const MB_FORMAT = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 0 });

/** Memória em MB como número e unidade: `{ value: '4,2', unit: 'GB' }`, `{ value: '512', unit: 'MB' }`. */
export function memoryParts(mb: number): { value: string; unit: string } {
  if (mb < 1024) {
    return { value: MB_FORMAT.format(mb), unit: t('unidade.mb') };
  }
  return { value: GB_FORMAT.format(mb / 1024), unit: t('unidade.gb') };
}

/** Memória em texto corrido: "4,2 GB". */
export function memoryText(mb: number): string {
  const { value, unit } = memoryParts(mb);
  return `${value} ${unit}`;
}

/** Um tempo de carregamento: "1 min 42 s"; abaixo de 1 s, "menos de 1 s". */
export function loadTimeText(ms: number): string {
  return ms < SAME_TIME_MS ? t('menosDeUmSegundo') : formatDuration(ms);
}

/** "18 s a mais que na 1.4.2", "6 s a menos que na 1.4.2" ou "o mesmo tempo da 1.4.2". */
export function comparisonText(loadedMs: number, previous: PreviousLoad): string {
  const delta = loadedMs - previous.loadedMs;
  const versao = previous.packVersion;
  if (Math.abs(delta) < SAME_TIME_MS) {
    return t('comparacao.igual', { versao });
  }
  const diferenca = formatDuration(Math.abs(delta));
  return delta > 0
    ? t('comparacao.mais', { diferenca, versao })
    : t('comparacao.menos', { diferenca, versao });
}
