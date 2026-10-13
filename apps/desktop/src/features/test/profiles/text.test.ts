import { describe, expect, it } from 'vitest';

import { makeProfile } from './profiles.fixtures';
import {
  memoryAboveRam,
  profileName,
  profileSummary,
  removePerfFlags,
  shortProfileName,
} from './text';

describe('textos dos perfis do teste', () => {
  it('nome mostrado e nome curto do botão Testar (CA-T11-04)', () => {
    expect(profileName(makeProfile('default', ''))).toBe('Padrão');
    expect(shortProfileName(makeProfile('perfil-1', 'PC fraco (4 GB)'))).toBe('PC fraco');
    expect(shortProfileName(makeProfile('perfil-2', 'Shaders'))).toBe('Shaders');
    // Só o complemento do fim sai; um nome todo entre parênteses fica como está.
    expect(shortProfileName(makeProfile('perfil-3', '(teste)'))).toBe('(teste)');
    expect(shortProfileName(makeProfile('perfil-4', 'A (b) c'))).toBe('A (b) c');
  });

  it('resumo do menu ▾', () => {
    expect(profileSummary(makeProfile('default', ''))).toBe('Memória automática · Java automático');
    expect(
      profileSummary(
        makeProfile('perfil-1', 'x', {
          memory: { mode: 'fixed', mb: 6144 },
          java: 'temurin-21',
          onLaunch: { mode: 'world', world: null },
        }),
      ),
    ).toBe('6 GB · Java escolhido · entra no mundo');
  });

  it('aviso de memória acima de 75% da RAM', () => {
    expect(memoryAboveRam({ mode: 'fixed', mb: 12_289 }, 16_384)).toBe(true);
    expect(memoryAboveRam({ mode: 'fixed', mb: 12_288 }, 16_384)).toBe(false);
    expect(memoryAboveRam({ mode: 'auto' }, 1024)).toBe(false);
    expect(memoryAboveRam({ mode: 'fixed', mb: 65_536 }, null)).toBe(false);
  });

  it('remove só os argumentos que impedem ler a memória, mantendo o resto como foi digitado', () => {
    expect(removePerfFlags('-XX:+PerfDisableSharedMem')).toBe('');
    expect(
      removePerfFlags('-XX:+UseG1GC  -XX:-UsePerfData -Dnome="a  b" -XX:+PerfDisableSharedMem'),
    ).toBe('-XX:+UseG1GC -Dnome="a  b"');
    // Parecidos não saem.
    expect(removePerfFlags('-XX:+PerfDisableSharedMemX -XX:-UsePerfDataY')).toBe(
      '-XX:+PerfDisableSharedMemX -XX:-UsePerfDataY',
    );
  });
});
