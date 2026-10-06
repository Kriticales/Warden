import { describe, expect, it } from 'vitest';

import { checkJvmArgs, splitArgs } from './jvm-args';

describe('argumentos extras da JVM', () => {
  it('separa por espaços e respeita aspas', () => {
    expect(splitArgs('  -Xss4M   -Dnome="Vale Sereno" \'-Da=b c\' ')).toEqual({
      args: ['-Xss4M', '-Dnome=Vale Sereno', '-Da=b c'],
      unbalanced: false,
    });
    expect(splitArgs('-Da="x')).toEqual({ args: ['-Da=x'], unbalanced: true });
    expect(splitArgs('')).toEqual({ args: [], unbalanced: false });
  });

  it('argumentos comuns não geram aviso', () => {
    const common =
      '-XX:+UseG1GC -XX:MaxGCPauseMillis=50 -Dfml.readTimeout=180 -Xss2M -javaagent:x.jar -ea';
    expect(checkJvmArgs(common, 17)).toEqual([]);
    expect(checkJvmArgs('--add-opens java.base/java.lang=ALL-UNNAMED', 17)).toEqual([]);
    expect(checkJvmArgs('', null)).toEqual([]);
  });

  it('memória pelo argumento é ignorada: a memória é a de "Memória do teste"', () => {
    expect(checkJvmArgs('-Xmx8G -Xms2048m', 17)).toEqual([
      { kind: 'memoria', arg: '-Xmx8G' },
      { kind: 'memoria', arg: '-Xms2048m' },
    ]);
  });

  it('argumentos que impedem a leitura da memória do jogo (ADR-0038)', () => {
    expect(checkJvmArgs('-XX:+PerfDisableSharedMem -XX:-UsePerfData', 21)).toEqual([
      { kind: 'leituraMemoria', arg: '-XX:+PerfDisableSharedMem' },
      { kind: 'leituraMemoria', arg: '-XX:-UsePerfData' },
    ]);
  });

  it('conferidos contra a versão do Java', () => {
    expect(checkJvmArgs('-XX:+UseConcMarkSweepGC', 17)).toEqual([
      { kind: 'removido', arg: '-XX:+UseConcMarkSweepGC', major: 17 },
    ]);
    // No Java 8 o CMS ainda existe.
    expect(checkJvmArgs('-XX:+UseConcMarkSweepGC -XX:+UseParNewGC', 8)).toEqual([]);
    expect(checkJvmArgs('-XX:MaxPermSize=256m', 21)).toEqual([
      { kind: 'removido', arg: '-XX:MaxPermSize=256m', major: 21 },
    ]);
    expect(checkJvmArgs('--add-opens java.base/java.lang=ALL-UNNAMED', 8)).toEqual([
      { kind: 'naoNoJava8', arg: '--add-opens' },
    ]);
  });

  it('o que a JVM não reconhece vira aviso; aspas abertas também', () => {
    expect(checkJvmArgs('-XX:+UseG1GC Xmx4G --modo-turbo', 17)).toEqual([
      { kind: 'desconhecido', arg: 'Xmx4G' },
      { kind: 'desconhecido', arg: '--modo-turbo' },
    ]);
    expect(checkJvmArgs('-Da="sem fim', 17)).toEqual([{ kind: 'aspas' }]);
  });
});
