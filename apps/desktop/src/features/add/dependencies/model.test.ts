import { describe, expect, it } from 'vitest';

import { makeNode, makePlan } from '../add.fixtures';
import { applyRequest, includedKeys, joinNames, NO_CHOICES, titlesOf } from './model';

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
        replaces: 'mods/modmenu.pw.toml',
      },
    ]);
  });
});
