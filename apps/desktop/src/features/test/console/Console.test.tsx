/**
 * Console do jogo (SPEC T13): linhas do Warden traduzidas e as do jogo como saíram, filtros,
 * cópia, pausa com contagem de linhas novas e virtualização com 50.000 linhas.
 */
import { act, fireEvent, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ConsoleLine } from '../../../lib/ipc/bindings';
import { axeComponent } from '../../../test/axe';
import { renderWithProviders } from '../../../test/render';
import { Console, type ConsoleProps } from './Console';

function gameLine(seq: number, overrides: Partial<ConsoleLine> = {}): ConsoleLine {
  return {
    seq,
    atMs: null,
    time: '19:53:26',
    level: 'info',
    logger: 'minecraft/Minecraft',
    thread: 'Render thread',
    text: `Linha ${String(seq)} do jogo`,
    params: {},
    origin: 'game',
    stream: 'stdout',
    ...overrides,
  };
}

function manyLines(count: number, from = 1): ConsoleLine[] {
  return Array.from({ length: count }, (_, i) => gameLine(from + i));
}

const wardenLine: ConsoleLine = {
  seq: 1,
  atMs: null,
  time: null,
  level: 'error',
  logger: null,
  thread: null,
  text: 'console.travou',
  params: { duracaoMs: '102000', codigo: '1' },
  origin: 'warden',
  stream: null,
};

/** Renderiza o console; `rerenderWith` troca as props sem remontar (como um lote novo). */
function renderConsole(props: Partial<ConsoleProps> = {}) {
  let update: (next: Partial<ConsoleProps>) => void = () => undefined;
  function Harness() {
    const [current, setCurrent] = useState<ConsoleProps>({ lines: [], state: 'live', ...props });
    update = (next) => {
      setCurrent((previous) => ({ ...previous, ...next }));
    };
    return <Console {...current} />;
  }
  const result = renderWithProviders(<Harness />);
  return {
    ...result,
    rerenderWith: (next: Partial<ConsoleProps>) => {
      act(() => {
        update(next);
      });
    },
  };
}

const theLog = () => screen.getByRole('log', { name: 'Console do jogo' });

/**
 * O jsdom não mede elementos (`offsetHeight` é 0). A lista virtualizada precisa da altura do
 * log (600 px nos testes) e da de cada linha medida (20 px).
 */
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('console__log')) return 600;
    return this.classList.contains('console__line') ? 20 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.classList.contains('console__log') ? 1000 : 0;
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Console', () => {
  it('mostra a linha do Warden traduzida e a do jogo como saiu, com acentos', () => {
    renderConsole({
      lines: [
        wardenLine,
        gameLine(2, { text: 'Gráficos avançados: ação não suportada', level: 'warn' }),
      ],
      state: 'ended',
    });
    const log = theLog();
    expect(log.getAttribute('aria-live')).toBe('off');
    expect(log.tabIndex).toBe(0);
    expect(
      within(log).getByText('O jogo fechou com o código 1 depois de 1 min 42 s.'),
    ).toBeDefined();
    const warden = within(log).getByText('WARDEN').closest('.console__line');
    expect(warden?.classList.contains('console__line--warden')).toBe(true);
    expect(within(log).getByText('Warden')).toBeDefined();
    const game = within(log).getByText('Gráficos avançados: ação não suportada');
    expect(game.closest('.console__line')?.classList.contains('console__line--warn')).toBe(true);
    expect(within(log).getByText('minecraft/Minecraft')).toBeDefined();
    expect(within(log).getByText('19:53:26')).toBeDefined();
  });

  it('sem linhas, explica; com filtro sem resultado, diz que não há linha', () => {
    const { rerenderWith } = renderConsole({ state: 'waiting' });
    expect(screen.getByText('Esperando o jogo abrir')).toBeDefined();
    expect(
      screen.getByText(
        'Nenhuma linha ainda. A saída do jogo aparece aqui assim que ele começar a abrir.',
      ),
    ).toBeDefined();
    rerenderWith({ lines: manyLines(3), state: 'live' });
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar no console' }), {
      target: { value: 'creeper' },
    });
    expect(screen.getByText('Nenhuma linha com esse filtro.')).toBeDefined();
    expect(screen.getByText('0 de 3 linhas')).toBeDefined();
  });

  it('filtra por nível e por busca, com a contagem', () => {
    renderConsole({
      lines: [
        gameLine(1, { text: 'Tudo certo' }),
        gameLine(2, { text: 'Textura faltando', level: 'warn' }),
        gameLine(3, { text: 'Falha no mixin', level: 'error', logger: 'mixin' }),
      ],
    });
    const log = theLog();
    const level = screen.getByRole('combobox', { name: 'Nível das linhas' });
    fireEvent.change(level, { target: { value: 'warn' } });
    expect(within(log).queryByText('Tudo certo')).toBeNull();
    expect(within(log).getByText('Textura faltando')).toBeDefined();
    expect(screen.getByText('2 de 3 linhas')).toBeDefined();
    fireEvent.change(level, { target: { value: 'error' } });
    expect(within(log).queryByText('Textura faltando')).toBeNull();
    expect(within(log).getByText('Falha no mixin')).toBeDefined();

    fireEvent.change(level, { target: { value: 'all' } });
    expect(screen.queryByText(/de 3 linhas/)).toBeNull();
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar no console' }), {
      target: { value: 'TEXTURA' },
    });
    expect(within(log).queryByText('Tudo certo')).toBeNull();
    // O trecho buscado fica destacado.
    expect(within(log).getByText('Textura').tagName).toBe('MARK');
    expect(screen.getByText('1 de 3 linhas')).toBeDefined();
  });

  it('"Copiar tudo" copia todas as linhas, uma por linha', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    renderConsole({
      lines: [
        gameLine(1, { text: 'Primeira' }),
        gameLine(2, { text: 'Segunda', level: 'warn', logger: 'fml' }),
        wardenLine,
      ],
      state: 'ended',
    });
    // O filtro não muda o que "Copiar tudo" leva.
    fireEvent.change(screen.getByRole('combobox', { name: 'Nível das linhas' }), {
      target: { value: 'error' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Copiar tudo' }));
    expect(writeText).toHaveBeenCalledWith(
      [
        '19:53:26 [INFO] [minecraft/Minecraft] Primeira',
        '19:53:26 [WARN] [fml] Segunda',
        '[WARDEN] [Warden] O jogo fechou com o código 1 depois de 1 min 42 s.',
      ].join('\n'),
    );
    expect(await screen.findByText('3 linhas copiadas.')).toBeDefined();
  });

  it('se copiar falhar, diz como copiar à mão', async () => {
    Object.assign(navigator, {
      clipboard: { writeText: () => Promise.reject(new Error('negado')) },
    });
    renderConsole({ lines: manyLines(2), state: 'ended' });
    fireEvent.click(screen.getByRole('button', { name: 'Copiar tudo' }));
    expect(
      await screen.findByText('Não foi possível copiar. Selecione o texto e use Ctrl+C.'),
    ).toBeDefined();
  });

  it('"Copiar seleção" só com texto selecionado no log; leva o trecho da mensagem', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    renderConsole({ lines: [gameLine(1, { text: 'Erro ao carregar o mundo' })], state: 'ended' });
    const button = screen.getByRole('button', { name: 'Copiar seleção' });
    expect(button).toHaveProperty('disabled', true);

    const message = within(theLog()).getByText('Erro ao carregar o mundo');
    const textNode = message.firstChild;
    if (!textNode) throw new Error('mensagem sem texto');
    const range = document.createRange();
    range.setStart(textNode, 0);
    range.setEnd(textNode, 4);
    const selection = document.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
    fireEvent(document, new Event('selectionchange'));
    expect(button).toHaveProperty('disabled', false);

    fireEvent.click(button);
    expect(writeText).toHaveBeenCalledWith('Erro');
    expect(await screen.findByText('Seleção copiada.')).toBeDefined();
    selection?.removeAllRanges();
  });

  it('pausado, conta as linhas novas; "Ir para o fim" retoma', () => {
    const first = manyLines(5);
    const { rerenderWith } = renderConsole({ lines: first });
    expect(screen.getByText('Ao vivo')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Pausar rolagem' }));
    expect(screen.getByText('Rolagem pausada')).toBeDefined();
    expect(screen.getByText('0 linha nova desde que você pausou.')).toBeDefined();

    rerenderWith({ lines: [...first, ...manyLines(3, 6)] });
    expect(screen.getByText('3 linhas novas desde que você pausou.')).toBeDefined();
    rerenderWith({ lines: [...first, ...manyLines(4, 6)] });
    expect(screen.getByText('4 linhas novas desde que você pausou.')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Ir para o fim' }));
    expect(screen.queryByText(/desde que você pausou/)).toBeNull();
    expect(screen.getByText('Ao vivo')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Pausar rolagem' })).toBeDefined();
  });

  it('rolar para cima com a roda pausa; "Retomar rolagem" também retoma', () => {
    renderConsole({ lines: manyLines(100) });
    const log = theLog();
    Object.defineProperty(log, 'scrollHeight', { configurable: true, value: 2000 });
    Object.defineProperty(log, 'clientHeight', { configurable: true, value: 600 });
    fireEvent.wheel(log, { deltaY: 120 });
    expect(screen.queryByText('Rolagem pausada')).toBeNull();
    fireEvent.wheel(log, { deltaY: -120 });
    expect(screen.getByText('Rolagem pausada')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Retomar rolagem' }));
    expect(screen.getByText('Ao vivo')).toBeDefined();
    fireEvent.keyDown(log, { key: 'PageUp' });
    expect(screen.getByText('Rolagem pausada')).toBeDefined();
    fireEvent.keyDown(log, { key: 'End' });
    expect(screen.getByText('Ao vivo')).toBeDefined();
  });

  it('com 50.000 linhas, só algumas dezenas ficam no DOM', () => {
    renderConsole({ lines: manyLines(50_000), state: 'ended' });
    const rendered = theLog().querySelectorAll('.console__line').length;
    expect(rendered).toBeGreaterThan(0);
    expect(rendered).toBeLessThan(150);
  });

  it('com o jogo fechado, sem pausa; sem onSave, sem "Salvar em arquivo…"', () => {
    const { rerenderWith } = renderConsole({ lines: manyLines(2), state: 'ended' });
    expect(screen.getByText('Jogo fechado')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Pausar rolagem' })).toBeNull();
    expect(screen.queryByRole('button', { name: /Salvar em arquivo/ })).toBeNull();

    const onSave = vi.fn();
    rerenderWith({ onSave });
    fireEvent.click(screen.getByRole('button', { name: 'Salvar em arquivo…' }));
    expect(onSave).toHaveBeenCalledOnce();
  });

  it('ao fechar o jogo pausado, a pausa some', () => {
    const { rerenderWith } = renderConsole({ lines: manyLines(3) });
    fireEvent.click(screen.getByRole('button', { name: 'Pausar rolagem' }));
    expect(screen.getByText('Rolagem pausada')).toBeDefined();
    rerenderWith({ state: 'ended' });
    expect(screen.getByText('Jogo fechado')).toBeDefined();
    expect(screen.queryByText(/desde que você pausou/)).toBeNull();
  });

  it('o rótulo muda o nome acessível', () => {
    renderConsole({ lines: manyLines(1), label: 'Console do servidor' });
    expect(screen.getByRole('region', { name: 'Console do servidor' })).toBeDefined();
    expect(screen.getByRole('log', { name: 'Console do servidor' })).toBeDefined();
  });

  it('sem violações de acessibilidade', async () => {
    const { container } = renderConsole({
      lines: [wardenLine, gameLine(2, { level: 'warn' })],
      onSave: () => undefined,
    });
    expect(await axeComponent(container)).toHaveNoViolations();
  });
});
