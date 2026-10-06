/**
 * Conferência dos argumentos extras da JVM (SPEC T11: "validados contra a versão do Java;
 * argumentos desconhecidos geram aviso"). Só avisa: quem decide é o usuário, e nada impede de
 * gravar. Funções puras.
 *
 * Fontes das remoções: notas de lançamento do OpenJDK (JEP 291/363 para o CMS, JEP 214 para o
 * ParNew e o `-Xincgc`, JDK 8 para o PermGen, JDK 11 para o `AggressiveOpts`).
 */

/** Um aviso sobre um argumento. */
export type JvmArgWarning =
  | { kind: 'memoria'; arg: string }
  | { kind: 'leituraMemoria'; arg: string }
  | { kind: 'removido'; arg: string; major: number }
  | { kind: 'naoNoJava8'; arg: string }
  | { kind: 'desconhecido'; arg: string }
  | { kind: 'aspas' };

/** Separa os argumentos como o shell faria: espaços separam, aspas agrupam. */
export function splitArgs(text: string): { args: string[]; unbalanced: boolean } {
  const args: string[] = [];
  let current = '';
  let quote: '"' | "'" | null = null;
  let started = false;
  for (const char of text) {
    if (quote) {
      if (char === quote) quote = null;
      else current += char;
      continue;
    }
    if (char === '"' || char === "'") {
      quote = char;
      started = true;
    } else if (/\s/.test(char)) {
      if (started) args.push(current);
      current = '';
      started = false;
    } else {
      current += char;
      started = true;
    }
  }
  if (started) args.push(current);
  return { args, unbalanced: quote !== null };
}

/** Argumentos que a memória do teste já define. */
const MEMORY = /^-Xm[xs]\d+[kKmMgG]?$/;

/** Impedem o Warden de ler a memória do jogo durante o teste (ADR-0038). */
const BLOCKS_MEMORY_READING = new Set(['-XX:+PerfDisableSharedMem', '-XX:-UsePerfData']);

/** Opção da JVM → major do Java em que deixou de existir. */
const REMOVED: readonly [RegExp, number][] = [
  [/^-XX:[+-]?(PermSize|MaxPermSize)(=.*)?$/, 17],
  [/^-Xincgc$/, 9],
  [/^-XX:[+-]UseParNewGC$/, 10],
  [/^-XX:[+-]AggressiveOpts$/, 12],
  [/^-XX:[+-]UseConcMarkSweepGC$/, 14],
  [/^-XX:[+-]CMSClassUnloadingEnabled$/, 14],
  [/^-XX:CMSInitiatingOccupancyFraction=\d+$/, 14],
];

/** Opções do sistema de módulos, que não existem no Java 8. */
const MODULES_ONLY = /^--(add-opens|add-exports|add-modules|add-reads|enable-native-access)(=.*)?$/;

/** Prefixos que a JVM aceita. O resto é provavelmente um erro de digitação. */
const KNOWN = [
  /^-X/,
  /^-D[^=]+(=.*)?$/,
  /^-(javaagent|agentlib|agentpath):/,
  /^-(ea|da|esa|dsa|enableassertions|disableassertions)(:.*)?$/,
  /^-(server|client|showversion|version)$/,
  /^-verbose(:.*)?$/,
  MODULES_ONLY,
  /^--enable-preview$/,
  /^--illegal-access=/,
];

/**
 * Os avisos dos argumentos para o Java `major` (o que o teste vai usar). Um argumento solto que
 * vem depois de `--add-opens`/`--add-exports` (o valor deles) não é conferido.
 */
export function checkJvmArgs(text: string, major: number | null): JvmArgWarning[] {
  const { args, unbalanced } = splitArgs(text);
  const warnings: JvmArgWarning[] = [];
  let expectValue = false;
  for (const arg of args) {
    if (expectValue) {
      expectValue = false;
      continue;
    }
    if (MEMORY.test(arg)) {
      warnings.push({ kind: 'memoria', arg });
      continue;
    }
    if (BLOCKS_MEMORY_READING.has(arg)) {
      warnings.push({ kind: 'leituraMemoria', arg });
      continue;
    }
    const removed = REMOVED.find(([pattern]) => pattern.test(arg));
    if (removed && major !== null && major >= removed[1]) {
      warnings.push({ kind: 'removido', arg, major });
      continue;
    }
    if (MODULES_ONLY.test(arg)) {
      if (major !== null && major <= 8) warnings.push({ kind: 'naoNoJava8', arg });
      // `--add-opens java.base/java.lang=ALL-UNNAMED`: o valor vem no argumento seguinte.
      expectValue = !arg.includes('=');
      continue;
    }
    if (!KNOWN.some((pattern) => pattern.test(arg))) {
      warnings.push({ kind: 'desconhecido', arg });
    }
  }
  if (unbalanced) warnings.push({ kind: 'aspas' });
  return warnings;
}
