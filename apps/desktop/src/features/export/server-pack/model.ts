/**
 * Contas do pacote para servidor que a tela mostra antes de gerar: as opções de memória do
 * `user_jvm_args.txt` e o valor de `-Xms`/`-Xmx` (o mesmo formato que o backend grava).
 */

/** Opções da memória do servidor, em MB, com a recomendada sempre presente e em ordem. */
const MEMORY_STEPS_MB = [2048, 3072, 4096, 6144, 8192, 12288, 16384] as const;

export function memoryOptions(recommendedMb: number): number[] {
  return [...new Set<number>([...MEMORY_STEPS_MB, recommendedMb])].sort((a, b) => a - b);
}

/** `4G` ou `3584M`, como no `user_jvm_args.txt`. */
export function memoryArg(memoryMb: number): string {
  return memoryMb % 1024 === 0 ? `${String(memoryMb / 1024)}G` : `${String(memoryMb)}M`;
}
