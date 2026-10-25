import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Ellipsis, Pencil, Save } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../test/axe';
import { renderWithProviders } from '../../test/render';
import { Alert } from './alert';
import { AlertDialog, AlertDialogCancel, AlertDialogContent } from './alert-dialog';
import { Button } from './button';
import { Dialog, DialogContent, DialogTrigger } from './dialog';
import { Menu, MenuContent, MenuIconTrigger, MenuItem } from './menu';
import { Segmented } from './segmented';
import { Sheet, SheetContent, SheetTrigger } from './sheet';
import { dismissToast, showToast, useToastStore } from './toast';
import { Tooltip, TooltipProvider } from './tooltip';

/**
 * Abre a camada por estado (sem `Trigger`, como as confirmações e os detalhes do mod), por um
 * botão ou pelo item de um menu ⋯ (que some com o menu).
 */
function OpenedByState({ layer }: { layer: (close: () => void) => ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setOpen(true);
        }}
      >
        Abrir pelo estado
      </button>
      <Menu>
        <MenuIconTrigger icon={Ellipsis} label="Mais ações" />
        <MenuContent>
          <MenuItem
            onSelect={() => {
              setOpen(true);
            }}
          >
            Abrir pelo menu
          </MenuItem>
        </MenuContent>
      </Menu>
      {open
        ? layer(() => {
            setOpen(false);
          })
        : null}
    </>
  );
}

/** Abre pelo botão e fecha com Esc: o foco volta ao botão. */
async function expectFocusBackFromButton(role: 'dialog' | 'alertdialog') {
  const user = userEvent.setup();
  const opener = screen.getByRole('button', { name: 'Abrir pelo estado' });
  await user.click(opener);
  await screen.findByRole(role);
  await user.keyboard('{Escape}');
  await waitFor(() => {
    expect(screen.queryByRole(role)).toBeNull();
  });
  // O Radix sozinho deixaria o foco no <body> (ele só devolve ao Trigger).
  await waitFor(() => {
    expect(document.activeElement).toBe(opener);
  });
}

/** Abre pelo item do menu ⋯, pelo teclado, e fecha com Esc: o foco volta ao ⋯. */
async function expectFocusBackFromMenu(role: 'dialog' | 'alertdialog') {
  const user = userEvent.setup();
  const trigger = screen.getByRole('button', { name: 'Mais ações' });
  trigger.focus();
  await user.keyboard('{Enter}');
  await screen.findByRole('menu');
  await user.keyboard('{Enter}');
  await screen.findByRole(role);
  await user.keyboard('{Escape}');
  await waitFor(() => {
    expect(screen.queryByRole(role)).toBeNull();
  });
  await waitFor(() => {
    expect(document.activeElement).toBe(trigger);
  });
}

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

describe('Segmented', () => {
  function Modes() {
    const [mode, setMode] = useState<'lista' | 'cartoes'>('lista');
    return (
      <Segmented
        label="Ver como"
        value={mode}
        options={[
          { value: 'lista', label: 'Lista' },
          { value: 'cartoes', label: 'Cartões', hint: 'Cartões com ícone' },
        ]}
        onChange={setMode}
      />
    );
  }

  it('grupo de rádio com uma parada do Tab; as setas trocam a opção e levam o foco; axe', async () => {
    const user = userEvent.setup();
    render(<Modes />);
    const lista = screen.getByRole('radio', { name: 'Lista' });
    const cartoes = screen.getByRole('radio', { name: 'Cartões' });
    expect(screen.getByRole('radiogroup', { name: 'Ver como' })).toBeDefined();
    expect(lista.getAttribute('aria-checked')).toBe('true');
    expect(lista.tabIndex).toBe(0);
    expect(cartoes.tabIndex).toBe(-1);
    await user.tab();
    expect(document.activeElement).toBe(lista);
    await user.keyboard('{ArrowRight}');
    expect(cartoes.getAttribute('aria-checked')).toBe('true');
    expect(document.activeElement).toBe(cartoes);
    await user.keyboard('{ArrowRight}');
    expect(lista.getAttribute('aria-checked')).toBe('true');
    await user.click(cartoes);
    expect(cartoes.getAttribute('aria-checked')).toBe('true');
    expect(cartoes.getAttribute('title')).toBe('Cartões com ícone');
    expect(await axeComponent(document.body)).toHaveNoViolations();
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

  it('aberto por estado (sem DialogTrigger), o foco volta ao que estava focado ao abrir', async () => {
    const user = userEvent.setup();
    function ByState() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <input aria-label="Busca" />
          <button
            type="button"
            onClick={() => {
              setOpen(true);
            }}
          >
            Abrir pelo estado
          </button>
          <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent title="Por estado" />
          </Dialog>
        </>
      );
    }
    renderWithProviders(<ByState />);
    const opener = screen.getByRole('button', { name: 'Abrir pelo estado' });
    await user.click(opener);
    await screen.findByRole('dialog', { name: 'Por estado' });
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    // O Radix sozinho deixaria o foco no <body> (ele só devolve ao DialogTrigger).
    await waitFor(() => {
      expect(document.activeElement).toBe(opener);
    });
  });

  it('aberto por um item de menu, o foco volta ao gatilho do menu (o item sumiu com o menu)', async () => {
    renderWithProviders(
      <OpenedByState
        layer={(close) => (
          <Dialog
            open
            onOpenChange={(next) => {
              if (!next) close();
            }}
          >
            <DialogContent title="Pelo menu" />
          </Dialog>
        )}
      />,
    );
    await expectFocusBackFromMenu('dialog');
  });

  it('sem nada focado ao abrir, o foco volta para returnFocusTo', async () => {
    const user = userEvent.setup();
    function Unfocused() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <button type="button">Plano B</button>
          <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent
              title="Sem quem abriu"
              returnFocusTo={() => screen.getByRole('button', { name: 'Plano B', hidden: true })}
            />
          </Dialog>
        </>
      );
    }
    renderWithProviders(<Unfocused />);
    await screen.findByRole('dialog', { name: 'Sem quem abriu' });
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Plano B' }));
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

  it('aberto por estado (a linha de um mod) ou por item de menu, o foco volta para quem abriu', async () => {
    renderWithProviders(
      <OpenedByState
        layer={(close) => (
          <Sheet
            open
            onOpenChange={(next) => {
              if (!next) close();
            }}
          >
            <SheetContent title="Detalhes" />
          </Sheet>
        )}
      />,
    );
    await expectFocusBackFromButton('dialog');
    await expectFocusBackFromMenu('dialog');
  });
});

describe('AlertDialog', () => {
  it('aberta por estado ou por item de menu (Remover, Apagar…), o foco volta para quem abriu', async () => {
    renderWithProviders(
      <OpenedByState
        layer={(close) => (
          <AlertDialog
            open
            onOpenChange={(next) => {
              if (!next) close();
            }}
          >
            <AlertDialogContent
              title="Remover?"
              description="O arquivo vai para a Lixeira."
              footer={
                <AlertDialogCancel asChild>
                  <Button>Cancelar</Button>
                </AlertDialogCancel>
              }
            />
          </AlertDialog>
        )}
      />,
    );
    await expectFocusBackFromButton('alertdialog');
    await expectFocusBackFromMenu('alertdialog');
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
