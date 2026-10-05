import { describe, expect, it } from 'vitest';

import type {
  InstalledRuntime,
  JavaChoice,
  JavaChoiceReason,
  JavaDecision,
  JavaOverview,
  JavaVersion,
  PackJavaUse,
} from '../../../../lib/ipc/bindings';
import { java as texts } from '../../../../i18n/pt-BR/java';
import {
  buildRows,
  choiceReason,
  decisionReason,
  joinNames,
  unusedCount,
  versionText,
  whyNotNewest,
} from './java-table';

function version(major: number, security: number, patch = 0, build = 0): JavaVersion {
  return { major, minor: 0, security, patch, build };
}

function runtime(
  id: string,
  v: JavaVersion,
  extra: Partial<InstalledRuntime> = {},
): InstalledRuntime {
  return {
    id,
    source: 'temurin',
    version: v,
    vendor: 'Eclipse Adoptium',
    releaseName: id,
    arch: 'x64',
    home: `C:/runtimes/${id}`,
    launcher: `C:/runtimes/${id}/bin/javaw.exe`,
    java: `C:/runtimes/${id}/bin/java.exe`,
    installedAtMs: 0,
    updateCap: null,
    mojangComponent: null,
    archiveSha256: null,
    supersededBy: null,
    ...extra,
  };
}

function decision(
  reason: JavaChoiceReason,
  major: number,
  extra: Partial<JavaDecision> = {},
): JavaDecision {
  return {
    requirement: { major, maxUpdate: null },
    reason,
    ruleId: null,
    rangeLabel: null,
    explanation: 'motivo da tabela',
    newestMajor: 25,
    versionJsonMajor: null,
    ...extra,
  };
}

function packUse(
  name: string,
  minecraft: string,
  automatic: JavaDecision,
  installed: InstalledRuntime | null,
): PackJavaUse {
  const choice: JavaChoice = {
    major: automatic.requirement.major,
    maxUpdate: automatic.requirement.maxUpdate,
    reason: automatic.reason,
    runtime: installed,
    automatic,
    userChoiceUnavailable: false,
  };
  return { packId: name, name, minecraft, choice };
}

const range17 = decision('NEWEST_PROVEN_FOR_RANGE', 17, { rangeLabel: '1.17 a 1.20.4' });
const legacy8 = decision('FORGE_LEGACY_JAVA8', 8, { rangeLabel: 'até o 1.12.2' });

describe('versionText', () => {
  it('escreve como o Rust', () => {
    expect(versionText(version(8, 312, 0, 7))).toBe('8u312');
    expect(versionText(version(21, 12, 1, 1))).toBe('21.0.12.1');
    expect(versionText(version(17, 15))).toBe('17.0.15');
  });
});

describe('motivos', () => {
  it('CA-T11-03: Forge 1.20.1 mostra o motivo e "Por que não o Java 25?"', () => {
    expect(decisionReason(range17)).toEqual({
      key: 'motivo.NEWEST_PROVEN_FOR_RANGE',
      params: { range: '1.17 a 1.20.4', major: 17 },
    });
    expect(whyNotNewest(range17)).toBe(25);
  });

  it('CA-T11-03: Forge 1.7.10 diz que só abre no Java 8', () => {
    expect(decisionReason(legacy8).key).toBe('motivo.FORGE_LEGACY_JAVA8');
    expect(texts.motivo.FORGE_LEGACY_JAVA8).toContain('só abre no Java 8');
  });

  it('CA-T11-03: a versão mais nova do Minecraft não ganha explicação extra', () => {
    const newest = decision('NEWEST_AVAILABLE', 25, { rangeLabel: '26.x' });
    expect(whyNotNewest(newest)).toBeNull();
    expect(decisionReason(newest).key).toBe('motivo.NEWEST_AVAILABLE');
  });

  it('JSON da versão, com e sem javaVersion', () => {
    expect(decisionReason(decision('FROM_VERSION_JSON', 21, { versionJsonMajor: 21 }))).toEqual({
      key: 'motivo.FROM_VERSION_JSON',
      params: { required: 21 },
    });
    expect(decisionReason(decision('FROM_VERSION_JSON', 8)).key).toBe(
      'motivo.FROM_VERSION_JSON_ANTIGA',
    );
  });

  it('escolha do usuário prevalece no motivo da linha', () => {
    const pack = packUse('A', '1.20.1', range17, null);
    pack.choice.reason = 'USER_CHOICE';
    expect(choiceReason(pack.choice).key).toBe('motivo.USER_CHOICE');
    expect(decisionReason(decision('USER_CHOICE', 21)).key).toBe('motivo.USER_CHOICE');
  });

  it('toda chave de motivo existe no catálogo', () => {
    const reasons: JavaChoiceReason[] = [
      'USER_CHOICE',
      'FORGE_LEGACY_JAVA8',
      'FORGE_1165_OLD',
      'NEWEST_PROVEN_FOR_RANGE',
      'NEWEST_AVAILABLE',
      'FROM_VERSION_JSON',
    ];
    for (const reason of reasons) {
      const key = decisionReason(decision(reason, 17, { versionJsonMajor: 17 })).key;
      const name = key.replace('motivo.', '') as keyof typeof texts.motivo;
      expect(texts.motivo[name], key).toBeTruthy();
    }
  });
});

describe('buildRows (CA-T21-04)', () => {
  const java17 = runtime('temurin-17-a', version(17, 20, 1, 1));
  const java8 = runtime('temurin-8-a', version(8, 504, 0, 1));
  const oldJava17 = runtime('temurin-17-velho', version(17, 15), { supersededBy: java17.id });
  const unused21 = runtime('temurin-21-a', version(21, 12, 1, 1));

  const overview: JavaOverview = {
    runtimes: [
      { runtime: unused21, packs: [], inGame: false },
      {
        runtime: java17,
        packs: [
          packUse('Vale Sereno', '1.20.1', range17, java17),
          packUse('Create', '1.20.1', range17, java17),
        ],
        inGame: false,
      },
      { runtime: oldJava17, packs: [], inGame: true },
      {
        runtime: java8,
        packs: [packUse('Técnico Clássico', '1.7.10', legacy8, java8)],
        inGame: false,
      },
    ],
    packsToDownload: [
      packUse('Novo', '26.3', decision('NEWEST_AVAILABLE', 25, { rangeLabel: '26.x' }), null),
      packUse('Outro novo', '26.1', decision('NEWEST_AVAILABLE', 25, { rangeLabel: '26.x' }), null),
    ],
    unresolved: [],
    brokenCount: 0,
  };

  it('ordena do Java mais novo para o mais velho, com os packs e o motivo', () => {
    const rows = buildRows(overview);
    expect(rows.map((row) => row.key)).toEqual([
      'baixar-25-ultima',
      'temurin-21-a',
      'temurin-17-a',
      'temurin-17-velho',
      'temurin-8-a',
    ]);
    const [toDownload, java21, current17, old17, legacy] = rows;
    expect(toDownload?.state).toBe('naoBaixado');
    expect(toDownload?.packs.map((p) => p.name)).toEqual(['Novo', 'Outro novo']);
    expect(toDownload?.reasons).toHaveLength(1);
    expect(java21?.state).toBe('semUso');
    expect(current17?.state).toBe('emUso');
    expect(current17?.packs.map((p) => p.name)).toEqual(['Create', 'Vale Sereno']);
    expect(current17?.reasons).toEqual([
      { key: 'motivo.NEWEST_PROVEN_FOR_RANGE', params: { range: '1.17 a 1.20.4', major: 17 } },
    ]);
    expect(old17?.state).toBe('substituido');
    expect(old17?.supersededBy).toBe('17.0.20.1');
    expect(old17?.inGame).toBe(true);
    expect(legacy?.reasons[0]?.key).toBe('motivo.FORGE_LEGACY_JAVA8');
  });

  it('linha com motivos diferentes mostra cada um uma vez', () => {
    const vanilla = decision('NEWEST_PROVEN_FOR_RANGE', 8, { rangeLabel: 'até o 1.16.5' });
    const rows = buildRows({
      ...overview,
      runtimes: [
        {
          runtime: java8,
          packs: [
            packUse('A', '1.12.2', legacy8, java8),
            packUse('B', '1.16.5', vanilla, java8),
            packUse('C', '1.7.10', legacy8, java8),
          ],
          inGame: false,
        },
      ],
      packsToDownload: [],
    });
    expect(rows[0]?.reasons.map((r) => r.key)).toEqual([
      'motivo.FORGE_LEGACY_JAVA8',
      'motivo.NEWEST_PROVEN_FOR_RANGE',
    ]);
  });

  it('substituto que já sumiu mostra o id', () => {
    const rows = buildRows({
      ...overview,
      runtimes: [{ runtime: oldJava17, packs: [], inGame: false }],
      packsToDownload: [],
    });
    expect(rows[0]?.supersededBy).toBe(java17.id);
  });

  it('o mesmo major com e sem teto vira duas linhas a baixar', () => {
    const capped = decision('FORGE_1165_OLD', 8);
    capped.requirement.maxUpdate = 312;
    const rows = buildRows({
      runtimes: [],
      packsToDownload: [
        packUse('Antigo', '1.16.5', capped, null),
        packUse('Clássico', '1.7.10', legacy8, null),
      ],
      unresolved: [],
      brokenCount: 0,
    });
    expect(rows.map((row) => row.key)).toEqual(['baixar-8-312', 'baixar-8-ultima']);
  });

  it('conta os Javas sem uso (sem pack e sem jogo aberto)', () => {
    expect(unusedCount(overview)).toBe(1);
  });
});

describe('joinNames', () => {
  it('junta em português', () => {
    expect(joinNames(['A'])).toBe('A');
    expect(joinNames(['A', 'B'])).toBe('A e B');
    expect(joinNames(['A', 'B', 'C'])).toBe('A, B e C');
  });
});
