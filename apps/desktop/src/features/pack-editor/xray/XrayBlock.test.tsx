/**
 * "O que este mod altera no jogo" nos detalhes do item (SPEC T07; CA-T07-05 na interface; D-10),
 * com o backend simulado: resumo, sobreposições com o risco e o motivo, "Ver todas as
 * alterações" no mesmo painel, "Ver em Problemas" e a atualização quando o pack ou um teste muda.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  DiagnosticsReport,
  GameState,
  InventoryItem,
  XrayModView,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { renderApp } from '../../../test/render';
import { makeDetails, makeInventory, makeItem } from '../editor.fixtures';
import { editorBackend } from '../testing';
import { MAX_ROWS } from './XrayAll';
import { makeOverlap, makeParticipant, makePoint, makeReport, makeXrayView } from './xray.fixtures';

const EXTRA = makeItem({ name: 'Sodium Extra', path: 'mods/sodium-extra.pw.toml' });
const EXTRAS = makeItem({ name: 'Sodium Extras', path: 'mods/sodiumextras.pw.toml' });
const NAMES = { [EXTRA.path]: 'Sodium Extra', [EXTRAS.path]: 'Sodium Extras' };

function report(findingPaths: string[] = []): DiagnosticsReport {
  return {
    pass: 'complete',
    checkedAtMs: 0,
    itemCount: 2,
    checkedMods: true,
    packHash: null,
    stale: false,
    findings: findingPaths.length
      ? [
          {
            id: `W_MIXIN_OVERLAP|${findingPaths.join('|')}`,
            finding: {
              rule: 'W_MIXIN_OVERLAP',
              severity: 'warning',
              titleKey: 'diagnostics.pretest.w_mixin_overlap',
              params: {
                ponto: 'SodiumGameOptionPages.general',
                mais: '0',
                risco: 'alto',
                travamento: 'nao',
              },
              items: findingPaths.map((path) => ({ kind: 'metafile', path })),
              evidence: [
                {
                  kind: 'jarMetadata',
                  item: { kind: 'metafile', path: findingPaths[0] ?? '' },
                  fileInJar: 'sodium-extra.mixins.json',
                  excerpt: '@Redirect em SodiumGameOptionPages.general (INVOKE …), prioridade 1000',
                },
              ],
              fixes: [],
            },
            items: findingPaths.map((path) => ({ path, name: NAMES[path] ?? path })),
            pattern: null,
            labels: {},
            ignored: false,
          },
        ]
      : [],
  };
}

const HIGH = makeXrayView({
  names: NAMES,
  report: makeReport({
    configs: ['sodium-extra.mixins.json', 'sodium-extra.client.mixins.json'],
    classCount: 40,
    changeCount: 61,
    pointCount: 52,
    sharedPointCount: 2,
    priorities: [1000],
    overlaps: [
      makeOverlap(),
      makeOverlap({
        targetClass: 'net/minecraft/class_761',
        classNamed: 'net/minecraft/client/render/WorldRenderer',
        method: 'method_3',
        methodNamed: 'renderSky',
        rule: 'twoReplacements',
        base: 'high',
        risk: 'low',
        participants: [
          makeParticipant(EXTRA.path, { plugin: true }),
          makeParticipant(EXTRAS.path, { plugin: true }),
        ],
        demoters: [{ kind: 'allPlugin' }, { kind: 'knownCompatible', origin: 'S-R5-1' }],
      }),
    ],
    points: [
      makePoint({
        targetClass: 'me/jellysquid/mods/sodium/client/gui/SodiumGameOptionPages',
        method: 'general',
        kinds: ['redirect'],
        others: [EXTRAS.path],
        risk: 'high',
      }),
      makePoint({
        classNamed: 'net/minecraft/client/MinecraftClient',
        method: 'method_2',
        methodNamed: 'tick',
      }),
    ],
  }),
});

async function openDrawer(
  view: XrayModView,
  diagnostics: DiagnosticsReport | null = null,
  item: InventoryItem = EXTRA,
) {
  const setup = editorBackend({
    inventory: makeInventory([EXTRA, EXTRAS]),
    handlers: {
      item_details: () => makeDetails(item),
      xray_mod: () => view,
      diagnostics_report_get: () => diagnostics,
    },
  });
  renderApp(setup.url);
  await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
  await userEvent.setup().click(screen.getByRole('button', { name: item.name }));
  const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
  const block = await within(drawer).findByRole('region', {
    name: 'O que este mod altera no jogo',
  });
  return { ...setup, drawer, block };
}

describe('raio-x de mixins nos detalhes (D-10)', () => {
  it('resumo, sobreposição de risco alto com o motivo, rebaixadores e Ver em Problemas; axe', async () => {
    const { block, backend } = await openDrawer(HIGH, report([EXTRA.path, EXTRAS.path]));
    expect(
      await within(block).findByText('Altera 52 partes do jogo, em 2 configs.', { exact: false }),
    ).toBeDefined();
    expect(block.textContent).toContain('2 também alteradas por outros mods.');
    const [high, low] = within(block).getAllByRole('listitem');
    expect(high?.textContent).toContain('SodiumGameOptionPages.general');
    expect(high?.textContent).toContain('Risco alto');
    expect(high?.textContent).toContain('Sodium Extra @Redirect troca uma chamada');
    expect(high?.textContent).toContain('Sodium Extras @Redirect');
    expect(high?.textContent).toContain('Só um vale; o outro é pulado em silêncio.');
    // Nome legível ensinado pelo refmap.
    expect(low?.textContent).toContain('WorldRenderer.renderSky');
    expect(low?.textContent).toContain('Risco baixo');
    expect(low?.textContent).toContain('Pode ser desligado pelo próprio mod.');
    expect(low?.textContent).toContain('Par conhecido como compatível.');
    expect(block.textContent).toContain(
      '“Alteram o mesmo ponto” não quer dizer que não funcionam juntos',
    );
    expect(block.textContent).not.toMatch(/incompatíve/);
    const link = within(block).getByRole('link', { name: 'Ver em Problemas' });
    expect(link.getAttribute('href')).toContain('/problemas');
    expect(backend.callsOf('xray_mod')[0]?.args).toMatchObject({ path: EXTRA.path });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('sem aviso em Problemas para o item, não oferece Ver em Problemas', async () => {
    const { block } = await openDrawer(HIGH, report([]));
    await within(block).findByText(/Altera 52 partes/);
    expect(within(block).queryByRole('link', { name: 'Ver em Problemas' })).toBeNull();
  });

  it('mod sem mixins', async () => {
    const { block } = await openDrawer(makeXrayView());
    expect(
      await within(block).findByText('Este mod não altera o código do jogo por mixins.'),
    ).toBeDefined();
  });

  it('jar ainda não baixado pede para testar uma vez', async () => {
    const { block } = await openDrawer(makeXrayView({ state: 'noJar', report: null }));
    expect(await within(block).findByText(/Teste o pack uma vez/)).toBeDefined();
  });

  it('mod do Sinytra Connector num pack Forge e alterações desligadas por opções', async () => {
    const { block } = await openDrawer(
      makeXrayView({
        nativeLoader: true,
        report: makeReport({
          connector: true,
          changeCount: 3,
          pointCount: 3,
          configs: ['a.mixins.json'],
          disabledClassCount: 2,
        }),
      }),
    );
    expect(await within(block).findByText(/Sinytra Connector/)).toBeDefined();
    expect(block.textContent).toContain('2 alterações estão desligadas');
  });

  it('Ver todas as alterações: filtra por texto e por risco e Detalhes do mod devolve o foco', async () => {
    const user = userEvent.setup();
    const { block } = await openDrawer(HIGH);
    const open = await within(block).findByRole('button', { name: 'Ver todas as alterações' });
    await user.click(open);
    const all = await screen.findByRole('dialog', { name: 'Todas as alterações de Sodium Extra' });
    const rows = () => within(all).getAllByRole('row').slice(1);
    expect(rows()).toHaveLength(2);
    expect(all.textContent).toContain(
      '61 alterações. Configs: sodium-extra.mixins.json, sodium-extra.client.mixins.json.',
    );
    expect(all.textContent).toContain('prioridade 1000');
    expect(rows()[0]?.textContent).toContain('SodiumGameOptionPages.general');
    expect(rows()[0]?.textContent).toContain('Risco alto');
    expect(rows()[0]?.textContent).toContain('Sodium Extras');
    expect(rows()[1]?.textContent).toContain('MinecraftClient.tick');
    expect(rows()[1]?.textContent).toContain('nenhum');

    await user.type(
      within(all).getByRole('searchbox', { name: 'Filtrar por parte do jogo' }),
      'tick',
    );
    expect(rows()).toHaveLength(1);
    await user.clear(within(all).getByRole('searchbox', { name: 'Filtrar por parte do jogo' }));
    await user.selectOptions(within(all).getByLabelText('Mostrar'), 'Só risco alto');
    expect(rows()).toHaveLength(1);
    expect(rows()[0]?.textContent).toContain('SodiumGameOptionPages');
    await user.selectOptions(within(all).getByLabelText('Mostrar'), 'Só risco médio');
    expect(within(all).getByText('Nenhuma parte com esse filtro.')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(all).getByRole('button', { name: 'Detalhes do mod' }));
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'Todas as alterações de Sodium Extra' }),
      ).toBeNull();
    });
    expect(screen.getByRole('dialog', { name: 'Detalhes do mod' })).toBeDefined();
    await waitFor(() => {
      expect(document.activeElement).toBe(open);
    });
  });

  it('lista longa mostra as primeiras e manda filtrar', async () => {
    const points = Array.from({ length: MAX_ROWS + 7 }, (_, i) =>
      makePoint({ method: `method_${String(i)}` }),
    );
    const { block } = await openDrawer(
      makeXrayView({
        report: makeReport({
          changeCount: points.length,
          pointCount: points.length,
          points,
          configs: ['a.json'],
        }),
      }),
    );
    await userEvent
      .setup()
      .click(await within(block).findByRole('button', { name: 'Ver todas as alterações' }));
    const all = await screen.findByRole('dialog', { name: /Todas as alterações/ });
    expect(within(all).getByText('… e mais 7 partes. Use o filtro para achar.')).toBeDefined();
  });

  it('dados sempre atuais: relê quando o pack muda e quando um teste termina', async () => {
    const { block, backend, row } = await openDrawer(HIGH);
    await within(block).findByText(/Altera 52 partes/);
    const before = backend.callsOf('xray_mod').length;
    await act(() => backend.emit('pack-changed', { packId: row.id, areas: ['inventory'] }));
    await waitFor(() => {
      expect(backend.callsOf('xray_mod').length).toBeGreaterThan(before);
    });
    const afterChange = backend.callsOf('xray_mod').length;
    const exited: GameState = {
      packId: row.id,
      packName: row.name,
      state: 'exited',
      sessionId: 'sessao',
      operationId: null,
      profile: null,
      startedAtMs: 0,
    };
    await act(() => backend.emit('game-state', exited));
    await waitFor(() => {
      expect(backend.callsOf('xray_mod').length).toBeGreaterThan(afterChange);
    });
  });
});
