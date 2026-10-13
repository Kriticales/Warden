/**
 * Respostas do `xray_mod` para os testes (D-10). Os números e nomes imitam o corpus do S-R5-1
 * (sodium-extra × sodiumextras no pack Fabric 1.20.1).
 */
import type {
  ModReport,
  Overlap,
  Participant,
  PointRow,
  XrayModView,
} from '../../../lib/ipc/bindings';

export function makeReport(overrides: Partial<ModReport> = {}): ModReport {
  return {
    item: 'mods/sodium-extra.pw.toml',
    modIds: ['sodium-extra'],
    connector: false,
    configs: [],
    classCount: 0,
    disabledClassCount: 0,
    changeCount: 0,
    counts: [],
    pointCount: 0,
    sharedPointCount: 0,
    priorities: [],
    overlaps: [],
    points: [],
    issues: [],
    mixinSquared: false,
    ...overrides,
  };
}

export function makeXrayView(overrides: Partial<XrayModView> = {}): XrayModView {
  return {
    state: 'ready',
    report: makeReport(),
    names: {},
    nativeLoader: false,
    ...overrides,
  };
}

export function makeParticipant(item: string, overrides: Partial<Participant> = {}): Participant {
  const id = item.replace(/^mods\//, '').replace(/\.pw\.toml$/, '');
  return {
    item,
    modId: id,
    kind: 'redirect',
    mixinClass: `x/${id}/mixin/SodiumGameOptionPagesMixin`,
    config: `${id}.mixins.json`,
    priority: 1000,
    at: {
      value: 'INVOKE',
      target: 'Lme/jellysquid/mods/sodium/client/gui/options/OptionGroup$Builder;add()V',
    },
    plugin: false,
    ...overrides,
  };
}

export function makeOverlap(overrides: Partial<Overlap> = {}): Overlap {
  return {
    targetClass: 'me/jellysquid/mods/sodium/client/gui/SodiumGameOptionPages',
    method: 'general',
    classNamed: '',
    methodNamed: '',
    items: ['mods/sodium-extra.pw.toml', 'mods/sodiumextras.pw.toml'],
    rule: 'twoReplacements',
    base: 'high',
    risk: 'high',
    participants: [
      makeParticipant('mods/sodium-extra.pw.toml'),
      makeParticipant('mods/sodiumextras.pw.toml'),
    ],
    demoters: [],
    elevators: [],
    ...overrides,
  };
}

export function makePoint(overrides: Partial<PointRow> = {}): PointRow {
  return {
    targetClass: 'net/minecraft/class_1',
    method: 'method_2',
    classNamed: '',
    methodNamed: '',
    kinds: ['inject'],
    others: [],
    risk: null,
    ...overrides,
  };
}
