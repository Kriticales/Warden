import { describe, expect, it } from 'vitest';

import { makeNode, makePlan } from '../add.fixtures';
import {
  applyRequest,
  attentionItems,
  includedKeys,
  joinNames,
  NO_CHOICES,
  planTree,
  roleCounts,
  rowState,
  setIncluded,
  titlesOf,
  unknownSide,
  withKnownTitles,
} from './model';

const MODMENU = 'modrinth:mOgUt4GM';
const APPLESKIN = 'modrinth:EsAfCjCV';
const FABRIC_API = 'modrinth:P7dR8mSH';
const PLACEHOLDER = 'modrinth:eXts2L7r';

describe('diálogo de dependências: o que entra', () => {
  it('CA-T09-05: a biblioteca comum aos dois entra uma vez, e a gravação tem cada item uma vez', () => {
    const plan = makePlan();
    const included = includedKeys(plan, NO_CHOICES);
    expect([...included]).toEqual([MODMENU, APPLESKIN, FABRIC_API, PLACEHOLDER]);
    const request = applyRequest(plan, included, NO_CHOICES);
    expect(request.items.map((item) => item.projectId)).toEqual([
      'mOgUt4GM',
      'EsAfCjCV',
      'P7dR8mSH',
      'eXts2L7r',
    ]);
    expect(request.items[2]).toEqual({
      source: 'modrinth',
      projectId: 'P7dR8mSH',
      versionId: 'V-FABRIC-API',
      side: 'client',
      replaces: null,
    });
    expect(joinNames(titlesOf(plan, plan.nodes[2]?.requiredBy ?? []))).toBe('Mod Menu e AppleSkin');
  });

  it('desmarcar um escolhido tira as obrigatórias só dele; a comum fica pelo outro', () => {
    const plan = makePlan();
    const included = includedKeys(plan, { ...NO_CHOICES, unchecked: new Set([MODMENU]) });
    expect([...included]).toEqual([APPLESKIN, FABRIC_API]);
  });

  it('obrigatória desmarcada sai; opcional só entra marcada, com as obrigatórias dela', () => {
    const plan = makePlan({
      nodes: [
        makeNode(),
        makeNode({
          projectId: 'OPT',
          title: 'Opcional',
          role: 'optional',
          optionalFor: [MODMENU],
        }),
        makeNode({
          projectId: 'LIBOPT',
          title: 'Lib da opcional',
          role: 'required',
          requiredBy: ['modrinth:OPT'],
        }),
        makeNode({ projectId: 'LIB', title: 'Lib', role: 'required', requiredBy: [MODMENU] }),
        makeNode({
          projectId: 'LIBLIB',
          title: 'Lib da lib',
          role: 'required',
          requiredBy: ['modrinth:LIB'],
        }),
      ],
    });
    expect([...includedKeys(plan, NO_CHOICES)]).toEqual([
      MODMENU,
      'modrinth:LIB',
      'modrinth:LIBLIB',
    ]);
    // Desmarcar a lib tira também a lib dela (fecho transitivo).
    expect([
      ...includedKeys(plan, { ...NO_CHOICES, unchecked: new Set(['modrinth:LIB']) }),
    ]).toEqual([MODMENU]);
    // Marcar a opcional traz a obrigatória dela.
    expect([...includedKeys(plan, { ...NO_CHOICES, optional: new Set(['modrinth:OPT']) })]).toEqual(
      [MODMENU, 'modrinth:OPT', 'modrinth:LIBOPT', 'modrinth:LIB', 'modrinth:LIBLIB'],
    );
    // Opcional marcada, mas o mod que a pedia foi desmarcado: ela sai (e a lib dela também),
    // como a caixa desabilitada mostra; não conta no botão nem é gravada.
    const orphan = includedKeys(plan, {
      ...NO_CHOICES,
      unchecked: new Set([MODMENU]),
      optional: new Set(['modrinth:OPT']),
    });
    expect([...orphan]).toEqual([]);
    expect(applyRequest(plan, orphan, NO_CHOICES).items).toEqual([]);
  });

  it('duplicado entre fontes: só entra com "Substituir", e a gravação apaga o antigo', () => {
    const plan = makePlan({
      nodes: [makeNode()],
      duplicates: [
        {
          key: MODMENU,
          existingPath: 'mods/modmenu.pw.toml',
          existingTitle: 'Mod Menu',
          existingSource: 'curseforge',
          matchedBy: 'hash',
        },
      ],
    });
    expect(includedKeys(plan, NO_CHOICES).size).toBe(0);
    const choices = { ...NO_CHOICES, replace: new Set([MODMENU]) };
    const included = includedKeys(plan, choices);
    expect(applyRequest(plan, included, choices).items).toEqual([
      {
        source: 'modrinth',
        projectId: 'mOgUt4GM',
        versionId: 'V-MODMENU',
        side: 'client',
        replaces: 'mods/modmenu.pw.toml',
      },
    ]);
  });
});

describe('diálogo Adicionar itens: árvore, situação e conta', () => {
  it('planTree: um galho por escolhido, o que só ele pede pendurado, "Em comum" uma vez', () => {
    const plan = makePlan({
      installed: [
        {
          key: 'modrinth:INST',
          title: 'Já no pack',
          path: 'mods/inst.pw.toml',
          packSource: 'modrinth',
          chosen: false,
          requiredBy: ['modrinth:eXts2L7r'],
        },
      ],
    });
    const tree = planTree(plan);
    const keys = (rows: readonly { item: { key: string }; depth: number }[]) =>
      rows.map((row) => `${row.item.key}@${String(row.depth)}`);
    expect(tree.branches.map(keys)).toEqual([
      // Mod Menu traz a Text Placeholder API, que traz o que já está no pack.
      [`${MODMENU}@0`, `${PLACEHOLDER}@1`, 'modrinth:INST@2'],
      [`${APPLESKIN}@0`],
    ]);
    // A Fabric API, pedida pelos dois, fica em "Em comum", uma vez só.
    expect(tree.common.map(keys)).toEqual([[`${FABRIC_API}@0`]]);
  });

  it('planTree: um ciclo declarado não trava e nada aparece duas vezes', () => {
    const plan = makePlan({
      nodes: [
        makeNode(),
        makeNode({ projectId: 'A', title: 'A', role: 'required', requiredBy: ['modrinth:B'] }),
        makeNode({ projectId: 'B', title: 'B', role: 'required', requiredBy: ['modrinth:A'] }),
      ],
    });
    const tree = planTree(plan);
    const all = [...tree.branches, ...tree.common].flat().map((row) => row.item.key);
    expect(all.sort()).toEqual(['modrinth:A', 'modrinth:B', MODMENU].sort());
  });

  it('roleCounts: a mesma conta por papel que o título, o botão e o rodapé mostram', () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes,
        makeNode({ projectId: 'OPT', title: 'Opcional', role: 'optional', optionalFor: [MODMENU] }),
      ],
    });
    const choices = { ...NO_CHOICES, optional: new Set(['modrinth:OPT']) };
    expect(roleCounts(plan, includedKeys(plan, choices))).toEqual({
      chosen: 2,
      required: 2,
      optional: 1,
      total: 5,
    });
  });

  it('rowState: uma situação por linha, a mais importante', () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes,
        makeNode({
          projectId: 'BETA',
          title: 'Beta',
          role: 'required',
          requiredBy: [APPLESKIN],
          channel: 'beta',
          outsideChannel: true,
        }),
      ],
      conflicts: [
        {
          item: { key: MODMENU, title: 'Mod Menu', path: null },
          other: { key: 'modrinth:X', title: 'X', path: 'mods/x.pw.toml' },
          withPack: true,
          declaredBy: MODMENU,
          reason: null,
        },
      ],
    });
    const node = (key: string) => {
      const found = plan.nodes.find((item) => item.key === key);
      if (!found) throw new Error(key);
      return found;
    };
    const unchecked = { ...NO_CHOICES, unchecked: new Set([APPLESKIN, FABRIC_API]) };
    const included = includedKeys(plan, unchecked);
    expect(rowState(plan, node(MODMENU), included, unchecked).tag).toBe('incompativel');
    expect(rowState(plan, node(APPLESKIN), included, unchecked)).toMatchObject({
      checked: false,
      tag: 'desmarcado',
      out: 'desmarcou',
    });
    // Obrigatória desmarcada enquanto o Mod Menu a pede.
    expect(rowState(plan, node(FABRIC_API), included, unchecked).tag).toBe('jogoPodeNaoAbrir');
    // Pedida só pelo AppleSkin, que saiu: não entra e a caixa fica desabilitada.
    expect(rowState(plan, node('modrinth:BETA'), included, unchecked)).toMatchObject({
      disabled: true,
      tag: 'naoEntra',
    });
    const all = includedKeys(plan, NO_CHOICES);
    expect(rowState(plan, node('modrinth:BETA'), all, NO_CHOICES).tag).toBe('beta');
  });

  it('attentionItems: incompatível que entra, duplicado e obrigatória desmarcada', () => {
    const plan = makePlan({
      conflicts: [
        {
          item: { key: MODMENU, title: 'Mod Menu', path: null },
          other: { key: 'modrinth:X', title: 'Sodium', path: 'mods/x.pw.toml' },
          withPack: true,
          declaredBy: MODMENU,
          reason: null,
        },
      ],
      duplicates: [
        {
          key: APPLESKIN,
          existingPath: 'mods/appleskin.pw.toml',
          existingTitle: 'AppleSkin',
          existingSource: 'curseforge',
          matchedBy: 'hash',
        },
      ],
    });
    const choices = { ...NO_CHOICES, unchecked: new Set([PLACEHOLDER]) };
    const included = includedKeys(plan, choices);
    expect(attentionItems(plan, included, choices).map((item) => [item.key, item.reason])).toEqual([
      [MODMENU, 'incompativel'],
      [APPLESKIN, 'duplicado'],
      [PLACEHOLDER, 'obrigatoria'],
    ]);
    // Sem o Mod Menu, a incompatibilidade dele não pede mais nada.
    const without = { ...choices, unchecked: new Set([MODMENU]) };
    expect(
      attentionItems(plan, includedKeys(plan, without), without).map((item) => item.reason),
    ).toEqual(['duplicado']);
  });

  it('setIncluded: tira e devolve escolhidos, obrigatórias e opcionais', () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes,
        makeNode({ projectId: 'OPT', title: 'Opcional', role: 'optional', optionalFor: [MODMENU] }),
      ],
    });
    const off = setIncluded(plan, NO_CHOICES, FABRIC_API, false);
    expect(includedKeys(plan, off).has(FABRIC_API)).toBe(false);
    expect(includedKeys(plan, setIncluded(plan, off, FABRIC_API, true)).has(FABRIC_API)).toBe(true);
    const optional = setIncluded(plan, NO_CHOICES, 'modrinth:OPT', true);
    expect(includedKeys(plan, optional).has('modrinth:OPT')).toBe(true);
    expect(unknownSide(plan, includedKeys(plan, optional))).toEqual([]);
  });
});

describe('nomes conhecidos', () => {
  it('withKnownTitles: o escolhido sem versão que veio só com o ID ganha o nome da busca', () => {
    const plan = makePlan({
      missing: [
        { key: 'modrinth:PtjYWJkn', title: 'PtjYWJkn', role: 'chosen', requiredBy: [] },
        { key: 'modrinth:OUTRO', title: 'Outro Nome', role: 'required', requiredBy: [MODMENU] },
      ],
    });
    const fixed = withKnownTitles(
      plan,
      [
        { source: 'modrinth', projectId: 'PtjYWJkn', versionId: null },
        { source: 'modrinth', projectId: 'OUTRO', versionId: null },
      ],
      ['Sodium Extra', 'Não troca'],
    );
    expect(fixed.missing.map((item) => item.title)).toEqual(['Sodium Extra', 'Outro Nome']);
    expect(titlesOf(fixed, ['modrinth:PtjYWJkn'])).toEqual(['Sodium Extra']);
    // Sem nada a trocar, o mesmo plano.
    const same = makePlan();
    expect(withKnownTitles(same, [], [])).toBe(same);
  });
});
