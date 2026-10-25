/**
 * Saúde do pack (SPEC T14 e T02; ROADMAP D-08) no app inteiro, com o backend simulado: a nota no
 * topo de Problemas com "O que tirou pontos" (CA-T14-07), a linha dos ignorados, "sem dados", a
 * coluna Saúde de Meus packs (CA-T02-05) e a nota sempre atual depois de "Ignorar neste pack",
 * "Verificar agora" e do fim de um teste. Os critérios estão no nome de cada teste.
 */
import { createMemoryHistory } from '@tanstack/react-router';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { App, createQueryClient } from '../../../app/App';
import type { HealthScore } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred, ipcError, mockBackend, type Handler } from '../../../test/backend';
import { makeAppError, makePackRow } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeInventory, makeItem } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import { invalidateAfterTest } from '../../test/api';
import { BALM_GROUP, makeCrashList, makeGroup } from '../crashes/crashes.fixtures';
import { crashTitle } from '../crashes/text';
import { MISSING_DEP, WRONG_SIDE, makeEntry, makeReport } from '../diagnostics.fixtures';
import { findingTitle } from '../text';
import { healthKeys } from './api';
import { HealthCell } from './HealthColumn';
import { CA_T14_07, finding, line, makeHealth } from './health.fixtures';

const inventory = makeInventory([
  makeItem({ path: 'mods/meu-mod.jar', name: 'Meu Mod', source: 'local', sideEditable: false }),
  makeItem({ path: 'mods/sodium.pw.toml', name: 'Sodium', key: 'modrinth:AANobbMI' }),
]);

const BLOCKED = makeEntry(
  { rule: 'W_DOWNLOAD_BLOCKED', severity: 'warning' },
  [{ path: 'mods/e.pw.toml', name: 'Create Extra' }],
  { id: 'W_DOWNLOAD_BLOCKED|mods/e.pw.toml' },
);

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    inventory,
    handlers: {
      diagnostics_report_get: () => makeReport({ findings: [MISSING_DEP, WRONG_SIDE, BLOCKED] }),
      diagnostics_run: () => makeReport(),
      crashes_list: () => makeCrashList({ groups: [BALM_GROUP, makeGroup()] }),
      health_get: () => CA_T14_07,
      ...handlers,
    },
  });
}

async function openHealth(url: string) {
  renderApp(`${url}/problemas`);
  const block = await screen.findByRole('region', { name: 'Saúde do pack' }, { timeout: 10_000 });
  await waitFor(() => {
    expect(within(block).queryByText('Calculando a saúde do pack…')).toBeNull();
  });
  return block;
}

/** As linhas de "O que tirou pontos" (sempre à vista no painel, como no esboço do bug-01). */
function losses(block: HTMLElement): Promise<HTMLElement[]> {
  expect(
    within(block).getByRole('heading', { level: 3, name: 'O que tirou pontos' }),
  ).toBeDefined();
  return Promise.resolve(within(block).getAllByRole('listitem'));
}

function sectionMenu() {
  return screen.getByRole('navigation', { name: 'Seções do pack' });
}

describe('saúde do pack em Problemas (T14)', () => {
  it('CA-T14-07: nota 33 (Crítico) com "O que tirou pontos" na ordem da tabela, links e axe', async () => {
    const { url } = setup();
    const block = await openHealth(url);
    const meter = within(block).getByRole('group', { name: 'Saúde do pack: 33 de 100, Crítico' });
    // A faixa tem palavra e ícone, não só cor.
    expect(within(meter).getByText('Crítico')).toBeDefined();
    expect(meter.textContent).toContain('33');
    expect(
      within(block).getByText('Resumo dos problemas conhecidos. Não garante que o pack funciona.'),
    ).toBeDefined();

    const rows = await losses(block);
    expect(rows.map((row) => row.querySelector('b')?.textContent)).toEqual([
      'Problemas encontrados:',
      'Último teste:',
      'Travamentos recentes:',
      'Mods que alteram o mesmo ponto do jogo:',
      'Download pelos jogadores:',
    ]);
    expect(rows.map((row) => row.querySelector('.losses__pts')?.textContent)).toEqual([
      '−33',
      '−20',
      '−6',
      '−4',
      '−4',
    ]);
    const [problems, lastTest, crashes, mixins, download] = rows as [
      HTMLElement,
      HTMLElement,
      HTMLElement,
      HTMLElement,
      HTMLElement,
    ];
    expect(problems.textContent).toContain('2 erros e 1 aviso: ');
    // Cada achado leva ao item na lista de Mods (o que não está no pack fica sem link).
    const missing = within(problems).getByRole('link', { name: findingTitle(MISSING_DEP) });
    expect(missing.getAttribute('href')).toContain('/mods?item=mods%2Fmeu-mod.jar');
    expect(
      within(problems)
        .getByRole('link', { name: findingTitle(WRONG_SIDE) })
        .getAttribute('href'),
    ).toContain('item=mods%2Fsodium.pw.toml');
    expect(within(problems).getAllByRole('link')).toHaveLength(2);

    expect(lastTest.textContent).toContain('o último teste travou.');
    expect(
      within(lastTest).getByRole('link', { name: 'Ver o teste' }).getAttribute('href'),
    ).toContain('/teste?sessao=2026-10-07_15-00-00');

    expect(crashes.textContent).toContain(
      '2 causas diferentes nas últimas 10 vezes que você testou.',
    );
    const causes = within(crashes).getAllByRole('link');
    expect(causes.map((link) => link.textContent)).toEqual([
      crashTitle(BALM_GROUP),
      crashTitle(makeGroup()),
    ]);
    expect(causes[1]?.getAttribute('href')).toContain('sessao=2026-10-07_14-20-00');

    expect(mixins.textContent).toContain('1 com risco alto: ');
    expect(download.textContent).toContain(
      '2 mods da CurseForge precisam ser baixados à mão. “Create Extra”; “f”',
    );
    expect(within(download).getAllByRole('link')[1]?.getAttribute('href')).toContain(
      'item=mods%2Ff.pw.toml',
    );
    expect(
      within(block).getByText(
        'Faixas: 90 a 100 Ótimo, 75 a 89 Bom, 50 a 74 Atenção, abaixo de 50 Crítico. Avisos que você mandar ignorar não tiram pontos.',
      ),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('item ignorado: linha riscada com "(ignorado por você)", sem desconto; teto avisado', async () => {
    const { url } = setup({
      health_get: () =>
        makeHealth({
          score: 60,
          band: 'attention',
          lines: [
            line(
              'problems',
              [
                finding(MISSING_DEP.id, 'E_MISSING_DEP', 'error', 15, 'mods/meu-mod.jar'),
                finding('E_A', 'E_MISSING_DEP', 'error', 15, null),
                finding('E_B', 'E_MISSING_DEP', 'error', 15, null),
              ],
              { points: 40, rawPoints: 45 },
            ),
            line(
              'problems',
              [finding(WRONG_SIDE.id, 'W_SIDE', 'warning', 3, 'mods/sodium.pw.toml')],
              { points: 0, rawPoints: 3, ignored: true },
            ),
          ],
        }),
    });
    const block = await openHealth(url);
    const [counted, ignored] = (await losses(block)) as [HTMLElement, HTMLElement];
    expect(counted.className).not.toContain('is-ignored');
    expect(counted.textContent).toContain('(o limite desta categoria)');
    expect(within(counted).getByLabelText('menos 40 pontos').textContent).toBe('−40');
    expect(ignored.className).toContain('is-ignored');
    expect(ignored.textContent).toContain('1 aviso: ');
    expect(ignored.textContent).toContain('(ignorado por você)');
    // Riscado com os pontos que tiraria, mas a nota não os conta.
    expect(within(ignored).getByLabelText('menos 3 pontos').textContent).toBe('−3');
    expect(
      within(block).getByRole('group', { name: 'Saúde do pack: 60 de 100, Atenção' }),
    ).toBeDefined();
    expect(await axePage(block)).toHaveNoViolations();
  });

  it('manutenção, desempenho e "alterado depois do último teste" com os links', async () => {
    const { url } = setup({
      health_get: () =>
        makeHealth({
          score: 84,
          band: 'good',
          lines: [
            line('lastTest', [
              { kind: 'lastTest', state: 'changedSince', session: 's-1', points: 5 },
            ]),
            line('maintenance', [
              finding(
                'W_OBSOLETE|mods/clumps.pw.toml',
                'W_OBSOLETE',
                'warning',
                2,
                'mods/clumps.pw.toml',
              ),
              finding(
                'I_PRERELEASE|mods/sodium.pw.toml',
                'I_PRERELEASE',
                'info',
                2,
                'mods/sodium.pw.toml',
              ),
              {
                kind: 'noNewVersion',
                item: 'mods/velho.pw.toml',
                name: 'Velho',
                months: 20,
                points: 2,
              },
            ]),
            line('performance', [
              {
                kind: 'slowLoad',
                session: 's-1',
                loadedMs: 90_000,
                goodSession: 's-0',
                goodMs: 60_000,
                points: 3,
              },
              { kind: 'lowMemory', session: 's-1', memoryMb: 2048, suggestedMb: 6144, points: 2 },
            ]),
          ],
        }),
    });
    const block = await openHealth(url);
    const [lastTest, maintenance, performance] = (await losses(block)) as [
      HTMLElement,
      HTMLElement,
      HTMLElement,
    ];
    expect(lastTest.textContent).toContain('o pack mudou depois do último teste.');
    expect(
      within(maintenance)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual([
      '“clumps” foi arquivado pelo autor',
      '“sodium” está numa versão alfa ou beta',
      '“Velho” está sem versão nova há 20 meses',
    ]);
    expect(
      within(maintenance)
        .getByRole('link', { name: '“Velho” está sem versão nova há 20 meses' })
        .getAttribute('href'),
    ).toContain('item=mods%2Fvelho.pw.toml');
    expect(performance.textContent).toContain(
      'o último teste levou 1 min 30 s para carregar, 50% a mais que o último teste bom (1 min).',
    );
    expect(performance.textContent).toContain(
      'a memória do último teste (2 GB) ficou abaixo da sugestão para a quantidade de mods (6 GB).',
    );
    expect(within(performance).getAllByRole('link', { name: 'Ver o teste' })).toHaveLength(2);
  });

  it('pack nunca verificado: "sem dados", nunca uma nota inventada', async () => {
    const running = deferred<never>();
    const { url } = setup({
      diagnostics_report_get: () => null,
      diagnostics_run: () => running.promise,
      health_get: () => null,
    });
    const block = await openHealth(url);
    expect(within(block).getByRole('group', { name: 'Saúde do pack: sem dados' })).toBeDefined();
    expect(within(block).getByText('Sem dados')).toBeDefined();
    expect(
      within(block).getByText('A saúde aparece depois da primeira verificação do pack.'),
    ).toBeDefined();
    expect(within(block).queryByText('O que tirou pontos')).toBeNull();
  });

  it('nota 100: "O que tirou pontos" diz que nada tirou; o selo diz quando verificou', async () => {
    const { url } = setup({ health_get: () => makeHealth() });
    const block = await openHealth(url);
    expect(
      within(block).getByRole('group', { name: 'Saúde do pack: 100 de 100, Ótimo' }),
    ).toBeDefined();
    expect(within(block).getByText('Nada tirou pontos.')).toBeDefined();
    expect(within(block).queryAllByRole('listitem')).toHaveLength(0);
    expect(within(block).getByText(/^Verificado /)).toBeDefined();
    // A barra tem 20 blocos, todos acesos com 100.
    expect(block.querySelectorAll('.health__blocks i.on')).toHaveLength(20);
  });

  it('erro ao calcular: a mensagem com "Tentar de novo"', async () => {
    let fail = true;
    const { url } = setup({
      health_get: () => {
        if (fail) {
          fail = false;
          ipcError(makeAppError());
        }
        return CA_T14_07;
      },
    });
    renderApp(`${url}/problemas`);
    const block = await screen.findByRole('region', { name: 'Saúde do pack' }, { timeout: 10_000 });
    expect(
      await within(block).findByText('Não foi possível calcular a saúde do pack.'),
    ).toBeDefined();
    await userEvent.setup().click(within(block).getByRole('button', { name: 'Tentar de novo' }));
    expect(
      await within(block).findByRole('group', { name: 'Saúde do pack: 33 de 100, Crítico' }),
    ).toBeDefined();
  });
});

describe('dados sempre atuais (topo de Problemas, Meus packs e contador do menu)', () => {
  /**
   * Backend que calcula a nota como o Rust: o aviso tira 3 pontos até ser ignorado, e cada
   * cálculo vira a nota guardada que `health_list` devolve.
   */
  function liveBackend() {
    let ignored = false;
    let checkedAtMs = Date.now() - 60_000;
    let stored: HealthScore | null = null;
    const report = () =>
      makeReport({ checkedAtMs, findings: [MISSING_DEP, { ...WRONG_SIDE, ignored }] });
    const health = (): HealthScore => {
      const reasons = [
        line('problems', [
          finding(MISSING_DEP.id, 'E_MISSING_DEP', 'error', 15, 'mods/meu-mod.jar'),
        ]),
        line('problems', [finding(WRONG_SIDE.id, 'W_SIDE', 'warning', 3, 'mods/sodium.pw.toml')], {
          ignored,
          points: ignored ? 0 : 3,
        }),
      ];
      const score = ignored ? 85 : 82;
      return makeHealth({ score, band: 'good', lines: reasons });
    };
    const setupResult = setup({
      diagnostics_report_get: report,
      diagnostics_run: () => {
        checkedAtMs = Date.now();
        return report();
      },
      diagnostics_ignore: (args) => {
        ignored = Boolean(args.ignored);
        return report();
      },
      health_get: () => {
        stored = health();
        return stored;
      },
      health_list: () =>
        stored
          ? [{ packId: setupResult.row.id, health: { score: stored.score, band: stored.band } }]
          : [],
    });
    return setupResult;
  }

  it('"Ignorar neste pack" devolve os pontos no topo, na linha de Meus packs e no contador do menu', async () => {
    const { url } = liveBackend();
    const queryClient = createQueryClient();
    // A lista de Meus packs já lida antes de abrir o pack.
    queryClient.setQueryData(healthKeys.list, new Map());
    render(
      <App
        history={createMemoryHistory({ initialEntries: [`${url}/problemas`] })}
        queryClient={queryClient}
      />,
    );
    const block = await screen.findByRole('region', { name: 'Saúde do pack' }, { timeout: 10_000 });
    expect(
      await within(block).findByRole('group', { name: 'Saúde do pack: 82 de 100, Bom' }),
    ).toBeDefined();
    expect(within(sectionMenu()).getByLabelText('2 problemas')).toBeDefined();
    const listAt = queryClient.getQueryState(healthKeys.list)?.dataUpdatedAt;

    const user = userEvent.setup();
    const card = screen.getByRole('article', { name: 'Sodium está no lado errado' });
    await user.click(within(card).getByRole('button', { name: 'Ignorar neste pack' }));
    expect(
      await within(block).findByRole('group', { name: 'Saúde do pack: 85 de 100, Bom' }),
    ).toBeDefined();
    expect(within(sectionMenu()).getByLabelText('1 problema')).toBeDefined();
    // O cálculo novo invalida a lista de Meus packs.
    await waitFor(() => {
      const state = queryClient.getQueryState(healthKeys.list);
      expect(state?.isInvalidated === true || state?.dataUpdatedAt !== listAt).toBe(true);
    });

    // De volta a Meus packs: a linha do pack tem a mesma nota.
    await user.click(screen.getByRole('link', { name: 'Meus packs' }));
    const title = await screen.findByRole('button', { name: 'Vale Sereno' }, { timeout: 10_000 });
    const row = title.closest('tr');
    if (!row) throw new Error('linha do pack');
    await waitFor(() => {
      expect(within(row).getByText('85')).toBeDefined();
    });
    expect(within(row).getByText('Bom')).toBeDefined();
  });

  it('"Verificar agora" e o fim de um teste recalculam a nota', async () => {
    const { backend, url } = liveBackend();
    const packId = url.split('/')[2] ?? '';
    const queryClient = createQueryClient();
    render(
      <App
        history={createMemoryHistory({ initialEntries: [`${url}/problemas`] })}
        queryClient={queryClient}
      />,
    );
    const block = await screen.findByRole('region', { name: 'Saúde do pack' }, { timeout: 10_000 });
    await within(block).findByRole('group', { name: 'Saúde do pack: 82 de 100, Bom' });
    const before = backend.callsOf('health_get').length;

    await userEvent.setup().click(screen.getByRole('button', { name: 'Verificar agora' }));
    await waitFor(() => {
      expect(backend.callsOf('health_get').length).toBe(before + 1);
    });

    // O fim de um teste relê as sessões (e os travamentos): a nota é calculada de novo.
    await invalidateAfterTest(queryClient, packId);
    await waitFor(() => {
      expect(backend.callsOf('health_get').length).toBe(before + 2);
    });
    expect(backend.callsOf('diagnostics_run')).toHaveLength(1);
  });
});

describe('coluna Saúde de Meus packs (T02)', () => {
  it('CA-T02-05: a nota e a faixa guardadas, "Sem dados" sem cálculo, sem rodar o diagnóstico; axe', async () => {
    const vale = makePackRow({ name: 'Vale Sereno' });
    const novo = makePackRow({ name: 'Pack Novo', path: 'C:/Packs/novo' });
    const backend = mockBackend({
      packs_list: () => [vale, novo],
      health_list: () => [{ packId: vale.id, health: { score: 33, band: 'critical' } }],
    });
    const { container } = renderApp('/packs');
    await screen.findByRole('button', { name: 'Vale Sereno' }, { timeout: 5000 });
    expect(screen.getByRole('columnheader', { name: 'Saúde' })).toBeDefined();
    const row = (name: string) => {
      const found = screen.getByRole('button', { name }).closest('tr');
      if (!found) throw new Error(name);
      return found;
    };
    await waitFor(() => {
      expect(within(row('Vale Sereno')).getByText('33')).toBeDefined();
    });
    expect(within(row('Vale Sereno')).getByText('Crítico')).toBeDefined();
    expect(within(row('Pack Novo')).getByText('Sem dados')).toBeDefined();
    expect(within(row('Pack Novo')).getByText('—')).toBeDefined();
    // Abrir a lista só lê as notas guardadas.
    for (const command of ['health_get', 'diagnostics_run', 'diagnostics_report_get']) {
      expect(backend.callsOf(command)).toHaveLength(0);
    }
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('CA-T02-01: a célula só mostra a nota que a tabela leu, sem consulta própria por linha', () => {
    // Sem QueryClient: uma célula que lesse as notas por conta própria quebraria aqui (51 linhas
    // com uma consulta cada deixavam a lista de 50 packs acima de 1 s).
    const cells = render(
      <table>
        <tbody>
          <tr>
            <HealthCell value={{ score: 88, band: 'good' }} />
            <HealthCell value={null} />
            <HealthCell value={undefined} />
          </tr>
        </tbody>
      </table>,
    );
    const [good, none, reading] = Array.from(cells.container.querySelectorAll('td'));
    expect(good?.textContent).toContain('88');
    expect(good?.textContent).toContain('Bom');
    expect(none?.textContent).toContain('Sem dados');
    expect(reading?.getAttribute('aria-busy')).toBe('true');
  });

  it('a dica do título diz o que a nota é', async () => {
    mockBackend({ packs_list: () => [makePackRow()] });
    renderApp('/packs');
    const header = await screen.findByRole('button', { name: 'Saúde' }, { timeout: 5000 });
    header.focus();
    expect(
      (
        await screen.findAllByText(
          'Resumo dos problemas conhecidos, de 0 a 100. Não garante que o pack funciona.',
        )
      ).length,
    ).toBeGreaterThan(0);
  });
});
