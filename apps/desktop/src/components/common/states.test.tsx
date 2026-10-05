import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../test/axe';
import { Button } from '../ui/button';
import { TooltipProvider } from '../ui/tooltip';
import { EmptyState } from './EmptyState';
import { LoadingState, Skeleton } from './LoadingState';
import { ProgressBar } from './ProgressBar';

describe('EmptyState', () => {
  it('título, texto, ações e dica, com a ilustração decorativa', async () => {
    const { container } = render(
      <TooltipProvider>
        <EmptyState
          title="Nenhum mod ainda"
          text="O pack está vazio. Adicione mods pela busca ou por um link."
          actions={<Button variant="primary">Adicionar mods</Button>}
          hint="Também dá para arrastar arquivos .jar para a janela."
          glyph="plus"
        />
      </TooltipProvider>,
    );
    expect(screen.getByRole('heading', { level: 2, name: 'Nenhum mod ainda' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Adicionar mods' })).toBeDefined();
    expect(container.querySelector('.empty__art')?.getAttribute('aria-hidden')).toBe('true');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('variantes ok, erro e compacta, com o nível de título pedido', () => {
    const { container, rerender } = render(<EmptyState kind="ok" title="Nenhum problema" />);
    expect(container.firstElementChild?.className).toContain('empty--ok');
    rerender(<EmptyState kind="error" compact headingLevel={3} title="Não abriu" />);
    expect(container.firstElementChild?.className).toContain('empty--error');
    expect(container.firstElementChild?.className).toContain('empty--compact');
    expect(screen.getByRole('heading', { level: 3 })).toBeDefined();
    // Símbolo do tom de erro (vermelho) na ilustração.
    expect(container.querySelector('rect[fill="#ff5c5c"]')).not.toBeNull();
  });
});

describe('LoadingState', () => {
  it('anuncia o que está carregando', async () => {
    const { container } = render(<LoadingState label="Lendo as tarefas…" />);
    const status = screen.getByRole('status');
    expect(status.textContent).toBe('Lendo as tarefas…');
    expect(status.className).toContain('loading--block');
    expect(container.querySelector('.loader')?.getAttribute('aria-hidden')).toBe('true');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('em linha e skeleton com a forma do conteúdo', () => {
    const { container } = render(
      <>
        <LoadingState inline label="Lendo…" />
        <Skeleton width="60%" />
        <Skeleton shape="tile" />
      </>,
    );
    expect(screen.getByRole('status').className).not.toContain('loading--block');
    const line = container.querySelector<HTMLElement>('.skeleton--line');
    expect(line?.style.getPropertyValue('--w')).toBe('60%');
    expect(container.querySelector('.skeleton--tile')?.getAttribute('aria-hidden')).toBe('true');
  });
});

describe('ProgressBar', () => {
  it('determinada: barra com valor, rótulo, porcentagem e linha de baixo', async () => {
    const { container } = render(
      <ProgressBar value={38.4} label="Baixando o Minecraft 1.20.1" meta="160 de 420 MB" />,
    );
    const bar = screen.getByRole('progressbar', { name: 'Baixando o Minecraft 1.20.1' });
    expect(bar.getAttribute('aria-valuenow')).toBe('38');
    expect(bar.getAttribute('aria-valuemax')).toBe('100');
    expect(screen.getByText('38%')).toBeDefined();
    expect(screen.getByText('160 de 420 MB')).toBeDefined();
    const fill = container.querySelector<HTMLElement>('.progress__fill');
    expect(fill?.style.getPropertyValue('--p')).toBe('38%');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('indeterminada: sem valor e com a classe do trecho andando', async () => {
    const { container } = render(<ProgressBar value={null} label="Conferindo arquivos" />);
    const bar = screen.getByRole('progressbar');
    expect(bar.hasAttribute('aria-valuenow')).toBe(false);
    expect(container.firstElementChild?.className).toContain('progress--indeterminate');
    expect(screen.queryByText(/%$/)).toBeNull();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('estados esperando, erro e concluída; valor fora da faixa é limitado; texto para leitor de tela', () => {
    const { container, rerender } = render(
      <ProgressBar value={140} label="x" state="done" valueText="Pronto" />,
    );
    expect(container.firstElementChild?.className).toContain('progress--done');
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100');
    expect(screen.getByRole('progressbar').getAttribute('aria-valuetext')).toBe('Pronto');
    rerender(<ProgressBar value={-3} label="x" state="error" size="sm" showValue={false} />);
    expect(container.firstElementChild?.className).toContain('progress--error');
    expect(container.firstElementChild?.className).toContain('progress--sm');
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
    expect(screen.queryByText('0%')).toBeNull();
    rerender(<ProgressBar value={10} label="x" state="waiting" />);
    expect(container.firstElementChild?.className).toContain('progress--waiting');
  });
});
