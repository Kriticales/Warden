/**
 * "Desempenho do último teste" na seção Problemas (SPEC T14 item 4; esboço aprovado do bug-01)
 * no app inteiro, com o backend simulado: os números, o aviso de versão mais pesada (T33), os
 * mods do spark, os estados vazios honestos e "✦ Perguntar ao Sculk por que ficou mais lento".
 * Também as duas colunas da seção e a chegada no Sensor Sculk com o assunto já preenchido.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import type { Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSources, makePreview } from '../ai/ai.fixtures';
import { makeReport } from '../diagnostics.fixtures';
import { PERF_SESSION_ID, makeLastTestPerf } from './perf.fixtures';
import { memoryOfText } from './text';

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    handlers: {
      diagnostics_report_get: () => makeReport({ findings: [] }),
      diagnostics_run: () => makeReport({ findings: [] }),
      perf_last_test: () => makeLastTestPerf(),
      ...handlers,
    },
  });
}

async function openPerf(url: string) {
  renderApp(`${url}/problemas`);
  const block = await screen.findByRole(
    'region',
    { name: 'Desempenho do último teste' },
    { timeout: 10_000 },
  );
  await waitFor(() => {
    expect(within(block).queryByText('Lendo o último teste…')).toBeNull();
  });
  return block;
}

/** O valor de um número pelo rótulo (`<dt>`). */
function metric(block: HTMLElement, label: string): string {
  const term = within(block).getByText(label, { selector: 'dt' });
  return term.nextElementSibling?.textContent ?? '';
}

describe('Desempenho do último teste (T14; bug-01)', () => {
  it('os números, a variação, "no limite", o tick, o aviso de mais lenta e os mods do spark; axe', async () => {
    const { url } = setup();
    const block = await openPerf(url);
    expect(metric(block, 'Tempo para abrir')).toBe('1 min 48 s+24% que na 0.1.1');
    expect(metric(block, 'Até entrar no mundo')).toBe('2 min 31 s');
    expect(metric(block, 'Memória máxima')).toBe('5,8 de 6 GBno limite');
    expect(metric(block, 'Tempo por tick (spark)')).toBe('38 ms');
    expect(
      within(block).getByText(
        'A versão 0.1.2 abre 24% mais lenta que a 0.1.1 (1 min 48 s contra 1 min 27 s), neste computador e com o mesmo perfil de teste.',
      ),
    ).toBeDefined();
    expect(
      within(block).getByRole('heading', { name: 'Mods que mais usaram processamento (spark)' }),
    ).toBeDefined();
    const mods = within(block).getAllByRole('listitem');
    expect(mods.map((row) => row.textContent)).toEqual([
      'Create21%',
      'Distant Horizons14%',
      'Iris9%',
    ]);

    expect(
      within(block)
        .getByRole('link', { name: /Ver o resultado do teste/ })
        .getAttribute('href'),
    ).toContain(`sessao=${PERF_SESSION_ID}`);
    const ask = within(block).getByRole('link', {
      name: 'Perguntar ao Sculk por que ficou mais lento',
    });
    const href = new URL(ask.getAttribute('href') ?? '', 'http://x');
    expect(href.pathname).toMatch(/\/ia$/);
    expect(href.searchParams.get('sessao')).toBe(PERF_SESSION_ID);
    expect(href.searchParams.get('sintoma')).toBe('slow');
    expect(href.searchParams.get('assunto')).toContain('abre 24% mais lenta que a 0.1.1');
    expect(ask.className).toContain('btn--ai');
    // O ✦ marca o que é do Sculk.
    expect(ask.querySelector('svg')).not.toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('sem teste que carregou: o estado vazio, sem números nem Sculk', async () => {
    const { url } = setup({ perf_last_test: () => null });
    const block = await openPerf(url);
    expect(within(block).getByText('Ainda não há teste que chegou a carregar.')).toBeDefined();
    expect(within(block).queryByRole('link')).toBeNull();
  });

  it('sem perfil do spark, sem leitura da memória do jogo e sem mundo; mais rápido que antes: sem "Perguntar ao Sculk"', async () => {
    const { url } = setup({
      perf_last_test: () =>
        makeLastTestPerf({
          loadedMs: 80_000,
          worldMs: null,
          memory: null,
          tick: null,
          sparkTop: null,
          comparison: {
            status: 'notHeavier',
            version: '0.1.2',
            previousVersion: '0.1.1',
            metric: null,
            current: null,
            previous: null,
            percent: null,
          },
        }),
    });
    const block = await openPerf(url);
    expect(metric(block, 'Tempo para abrir')).toBe('1 min 20 s−8% que na 0.1.1');
    expect(metric(block, 'Até entrar no mundo')).toBe('não entrou');
    // Sem a memória do jogo, o pico de RAM do processo.
    expect(metric(block, 'Memória máxima')).toBe('6,9 GBRAM do processo');
    expect(within(block).queryByText('Tempo por tick (spark)')).toBeNull();
    expect(within(block).getByText('Sem perfil do spark neste teste.')).toBeDefined();
    expect(within(block).queryByRole('alert')).toBeNull();
    expect(within(block).queryByText(/mais lenta/)).toBeNull();
    expect(
      within(block).queryByRole('link', { name: 'Perguntar ao Sculk por que ficou mais lento' }),
    ).toBeNull();
  });

  it('poucos testes para comparar: a frase da T33, sem aviso', async () => {
    const { url } = setup({
      perf_last_test: () =>
        makeLastTestPerf({
          comparison: {
            status: 'fewTests',
            version: '0.1.2',
            previousVersion: '0.1.1',
            metric: null,
            current: null,
            previous: null,
            percent: null,
          },
        }),
    });
    const block = await openPerf(url);
    expect(
      within(block).getByText(
        'Poucos testes para comparar com a 0.1.1. Teste de novo para confirmar.',
      ),
    ).toBeDefined();
    expect(within(block).queryByText(/abre 24% mais lenta/)).toBeNull();
  });

  it('as duas colunas do esboço: achados e travamentos na larga; saúde e desempenho na da direita', async () => {
    const { url } = setup();
    await openPerf(url);
    const columns = [...document.querySelectorAll('.diag-cols > .diag-cols__col')].map((column) =>
      [...column.querySelectorAll(':scope > section')].map(
        (section) =>
          document.getElementById(section.getAttribute('aria-labelledby') ?? '')?.textContent,
      ),
    );
    expect(columns).toEqual([
      ['Achados antes do teste', 'Travamentos'],
      ['Saúde do pack', 'Desempenho do último teste'],
    ]);
  });
});

describe('textos do desempenho', () => {
  it('memória "usado de máximo": o máximo como o usuário escolheu, sem casa decimal à toa', () => {
    expect(memoryOfText(5939, 6144)).toBe('5,8 de 6 GB');
    expect(memoryOfText(67, 4096)).toBe('67 MB de 4 GB');
    expect(memoryOfText(3000, 6656)).toBe('2,9 de 6,5 GB');
    expect(memoryOfText(300, 512)).toBe('300 MB de 512 MB');
  });
});

describe('Sensor Sculk com o assunto vindo de Problemas', () => {
  it('"✦ Perguntar ao Sculk" num achado: a conversa nova sobre o pack, com o assunto em "Contar algo", e o consentimento abre sozinho', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        secrets_status: () => ({
          curseforge: false,
          gemini: true,
          github: false,
        }),
        ai_sources_list: () => makeSources(),
        ai_conversation_preview: () => makePreview(),
      },
    });
    const assunto = 'Quero entender este achado: "Sodium está no lado errado".';
    renderApp(`${url}/ia?sintoma=slow&assunto=${encodeURIComponent(assunto)}`);
    await screen.findByRole('dialog', {}, { timeout: 10_000 });
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toEqual({
      symptom: 'slow',
      source: { kind: 'packOnly' },
      note: assunto,
      mode: null,
    });
  });
});
