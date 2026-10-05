import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Pencil, Save } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../test/axe';
import { renderWithProviders } from '../../test/render';
import { Alert } from './alert';
import { Button } from './button';
import { Dialog, DialogContent, DialogTrigger } from './dialog';
import { Sheet, SheetContent, SheetTrigger } from './sheet';
import { dismissToast, showToast, useToastStore } from './toast';
import { Tooltip, TooltipProvider } from './tooltip';

describe('Button', () => {
  it('variantes do design system, sem cantos arredondados', async () => {
    const { container } = render(
      <>
        <Button variant="primary" icon={Save} count={5}>
          Salvar versão
        </Button>
        <Button variant="danger-ghost" size="sm">
          Remover
        </Button>
        <Button variant="link">Ver problemas</Button>
      </>,
    );
    const primary = screen.getByRole('button', { name: /^Salvar versão/ });
    expect(primary.className).toBe('btn btn--primary');
    expect(primary.getAttribute('type')).toBe('button');
    expect(primary.querySelector('.btn__count')?.textContent).toBe('5');
    expect(screen.getByRole('button', { name: 'Remover' }).className).toContain('btn--sm');
    expect(container.innerHTML).not.toMatch(/rounded-/);
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('carregando: desabilitado, aria-busy e carregador no lugar do ícone', () => {
    render(
      <Button loading icon={Save}>
        Salvando…
      </Button>,
    );
    const button = screen.getByRole<HTMLButtonElement>('button', { name: 'Salvando…' });
    expect(button.disabled).toBe(true);
    expect(button.getAttribute('aria-busy')).toBe('true');
    expect(button.querySelector('.loader')).not.toBeNull();
  });

  it('só ícone: o texto é o nome acessível e aparece como tooltip no foco', async () => {
    const user = userEvent.setup();
    const { container } = render(
      <TooltipProvider>
        <Button iconOnly variant="ghost" icon={Pencil}>
          Editar informações
        </Button>
      </TooltipProvider>,
    );
    const button = screen.getByRole('button', { name: 'Editar informações' });
    expect(button.className).toContain('btn--icon');
    await user.tab();
    expect(document.activeElement).toBe(button);
    expect(await screen.findByRole('tooltip')).toBeDefined();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('asChild aplica a aparência ao filho (um link)', () => {
    render(
      <Button asChild variant="ghost">
        <a href="#x">Meus packs</a>
      </Button>,
    );
    expect(screen.getByRole('link', { name: 'Meus packs' }).className).toBe('btn btn--ghost');
  });
});

describe('Tooltip', () => {
  it('Esc esconde', async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <Tooltip content="Prioridade 1">
          <button type="button">P1</button>
        </Tooltip>
      </TooltipProvider>,
    );
    await user.tab();
    expect(await screen.findByRole('tooltip')).toBeDefined();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });
});

describe('Dialog', () => {
  it('foco no título, ✕ e Esc fecham, o foco volta para quem abriu', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <Dialog>
        <DialogTrigger asChild>
          <Button>Editar informações</Button>
        </DialogTrigger>
        <DialogContent
          title="Informações do pack"
          description="Nome, autor e descrição."
          footer={<Button variant="primary">Salvar</Button>}
        >
          <p>Corpo</p>
        </DialogContent>
      </Dialog>,
    );
    const trigger = screen.getByRole('button', { name: 'Editar informações' });
    await user.click(trigger);
    const dialog = await screen.findByRole('dialog', { name: 'Informações do pack' });
    expect(dialog.className).toBe('dialog');
    expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: 'Informações do pack' }),
    );
    expect(await axeComponent(document.body)).toHaveNoViolations();
    await user.click(screen.getByRole('button', { name: 'Fechar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(document.activeElement).toBe(trigger);
    await user.click(trigger);
    await screen.findByRole('dialog');
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('tamanhos e rodapé dividido', async () => {
    renderWithProviders(
      <Dialog open>
        <DialogContent title="Grande" size="lg" footer={<span>pé</span>} footerSplit />
      </Dialog>,
    );
    const dialog = await screen.findByRole('dialog');
    expect(dialog.className).toBe('dialog dialog--lg');
    expect(screen.getByText('pé').parentElement?.className).toBe(
      'dialog__foot dialog__foot--split',
    );
  });
});

describe('Sheet', () => {
  it('painel lateral com foco no título e Tab preso', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <Sheet>
        <SheetTrigger asChild>
          <Button>Abrir</Button>
        </SheetTrigger>
        <SheetContent title="Detalhes" footer={<Button>Ação</Button>}>
          <p>Conteúdo</p>
        </SheetContent>
      </Sheet>,
    );
    await user.click(screen.getByRole('button', { name: 'Abrir' }));
    const sheet = await screen.findByRole('dialog', { name: 'Detalhes' });
    expect(sheet.className).toBe('drawer');
    expect(document.activeElement?.textContent).toBe('Detalhes');
    await user.tab();
    await user.tab();
    await user.tab();
    expect(sheet.contains(document.activeElement)).toBe(true);
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });
});

describe('Alert', () => {
  it('perigo é role=alert; os outros não; IA usa o brilhinho', async () => {
    const { container, rerender } = render(
      <Alert kind="danger" title="Falhou" actions={<button type="button">Tentar</button>}>
        x
      </Alert>,
    );
    expect(screen.getByRole('alert')).toBeDefined();
    expect(await axeComponent(container)).toHaveNoViolations();
    rerender(<Alert kind="warn" title="Aviso" compact actionsBelow actions={<span>a</span>} />);
    expect(screen.queryByRole('alert')).toBeNull();
    expect(container.firstElementChild?.className).toContain('alert--compact');
    expect(container.querySelector('.alert__actions--below')).not.toBeNull();
    rerender(<Alert kind="ai" title="IA" role="status" />);
    expect(screen.getByRole('status')).toBeDefined();
    expect(container.querySelector('.lucide-sparkles')).not.toBeNull();
    rerender(<Alert kind="danger" title="Sem papel" role="none" />);
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('Toast', () => {
  it('aparece, tem o botão de fechar e some com dismiss', async () => {
    const user = userEvent.setup();
    const { container } = renderWithProviders(<div />);
    let id = 0;
    act(() => {
      id = showToast({ kind: 'ok', title: 'Versão 1.4.3 salva', text: '5 alterações' });
    });
    expect(await screen.findByText('Versão 1.4.3 salva')).toBeDefined();
    expect(screen.getByText('5 alterações')).toBeDefined();
    expect(container.ownerDocument.querySelector('.toast--ok')).not.toBeNull();
    expect(await axeComponent(container.ownerDocument.body)).toHaveNoViolations();
    act(() => {
      dismissToast(id);
    });
    await waitFor(() => {
      expect(screen.queryByText('Versão 1.4.3 salva')).toBeNull();
    });
    act(() => {
      showToast({
        title: 'Outro',
        action: { label: 'Desfazer', altText: 'Histórico', onClick: vi.fn() },
      });
    });
    await user.click(await screen.findByRole('button', { name: 'Fechar aviso' }));
    await waitFor(() => {
      expect(useToastStore.getState().toasts).toHaveLength(0);
    });
  });
});
