/**
 * O aviso `W_MIXIN_OVERLAP` na seção Problemas (SPEC T14; CA-T14-09 na interface; D-10): título
 * "alteram o mesmo ponto do jogo", o ponto e o risco em linguagem simples, a linha do travamento
 * quando ele subiu o risco, a evidência (configs e ponto) e o lugar dos avisos, que não impedem o
 * teste.
 */
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { DiagnosticsReport, ReportFinding } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { renderApp } from '../../../test/render';
import { makeInventory, makeItem } from '../editor.fixtures';
import { editorBackend } from '../testing';

const A = makeItem({ name: 'Sodium Extra', path: 'mods/sodium-extra.pw.toml' });
const B = makeItem({ name: 'Sodium Extras', path: 'mods/sodiumextras.pw.toml' });

function overlapFinding(params: Record<string, string>, crash: boolean): ReportFinding {
  const evidence: ReportFinding['finding']['evidence'] = [
    {
      kind: 'jarMetadata',
      item: { kind: 'metafile', path: A.path },
      fileInJar: 'sodium-extra.mixins.json',
      excerpt:
        '@Redirect em SodiumGameOptionPages.general (INVOKE OptionGroup$Builder.add), prioridade 1000',
    },
    {
      kind: 'jarMetadata',
      item: { kind: 'metafile', path: B.path },
      fileInJar: 'mixins.sodiumextras.json',
      excerpt:
        '@Redirect em SodiumGameOptionPages.general (INVOKE OptionGroup$Builder.add), prioridade 1000',
    },
  ];
  if (crash) {
    evidence.push({
      kind: 'log',
      file: 'output.log',
      line: 4120,
      excerpt: 'Mixin apply failed mixins.sodiumextras.json:SodiumGameOptionPagesMixin',
    });
  }
  return {
    id: `W_MIXIN_OVERLAP|${A.path}|${B.path}`,
    finding: {
      rule: 'W_MIXIN_OVERLAP',
      severity: 'warning',
      titleKey: 'diagnostics.pretest.w_mixin_overlap',
      params,
      items: [
        { kind: 'metafile', path: A.path },
        { kind: 'metafile', path: B.path },
      ],
      evidence,
      fixes: [],
    },
    items: [
      { path: A.path, name: A.name },
      { path: B.path, name: B.name },
    ],
    pattern: null,
    labels: {},
    ignored: false,
  };
}

function report(finding: ReportFinding): DiagnosticsReport {
  return {
    pass: 'complete',
    checkedAtMs: Date.now() - 60_000,
    itemCount: 2,
    checkedMods: true,
    packHash: null,
    stale: false,
    findings: [finding],
  };
}

async function openProblems(finding: ReportFinding) {
  const setup = editorBackend({
    inventory: makeInventory([A, B]),
    handlers: { diagnostics_report_get: () => report(finding) },
  });
  renderApp(`${setup.url}/problemas`);
  await screen.findByRole('heading', { level: 1, name: 'Problemas' }, { timeout: 10_000 });
  return screen.getByRole('region', { name: 'Avisos não impedem o teste' });
}

describe('aviso "alteram o mesmo ponto do jogo" em Problemas (D-10)', () => {
  it('CA-T14-09: risco alto pelo travamento, com as configs, o ponto e a linha do log; axe', async () => {
    const warnings = await openProblems(
      overlapFinding(
        { ponto: 'SodiumGameOptionPages.general', mais: '1', risco: 'alto', travamento: 'sim' },
        true,
      ),
    );
    const card = within(warnings).getByRole('article', {
      name: 'Sodium Extra e Sodium Extras alteram o mesmo ponto do jogo',
    });
    expect(card.textContent).toContain(
      'Os dois mudam SodiumGameOptionPages.general e mais 1 parte (risco alto). Muitas vezes é de propósito; se o jogo travar ou uma função de um deles sumir, comece por eles. O último teste travou citando a config de um deles. Este aviso nunca impede o teste.',
    );
    expect(card.textContent).not.toMatch(/incompatíve|\{\{|\$t\(/);
    await userEvent.setup().click(within(card).getByText('Como sabemos'));
    expect(card.textContent).toContain('sodium-extra.mixins.json');
    expect(card.textContent).toContain('mixins.sodiumextras.json');
    expect(card.textContent).toContain('@Redirect em SodiumGameOptionPages.general');
    expect(card.textContent).toContain('4.120');
    expect(card.textContent).toContain('Mixin apply failed mixins.sodiumextras.json');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('risco médio, um ponto só e sem travamento', async () => {
    const warnings = await openProblems(
      overlapFinding({ ponto: 'Level.tick', mais: '0', risco: 'medio', travamento: 'nao' }, false),
    );
    const card = within(warnings).getByRole('article', {
      name: 'Sodium Extra e Sodium Extras alteram o mesmo ponto do jogo',
    });
    expect(card.textContent).toContain(
      'Os dois mudam Level.tick (risco médio). Muitas vezes é de propósito; se o jogo travar ou uma função de um deles sumir, comece por eles. Este aviso nunca impede o teste.',
    );
  });
});
