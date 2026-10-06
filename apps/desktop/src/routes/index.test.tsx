import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axePage } from '../test/axe';
import { mockBackend } from '../test/backend';
import { renderApp } from '../test/render';

describe('tela inicial', () => {
  it('abre em Meus packs, com a moldura do app: barra, conteúdo e rodapé com Tarefas', async () => {
    mockBackend();
    renderApp('/');

    const main = await screen.findByRole('main');
    expect(
      await within(main).findByRole('heading', { level: 1, name: 'Meus packs' }),
    ).toBeDefined();
    expect(main.id).toBe('conteudo');
    expect(within(screen.getByRole('banner')).getByText('Warden')).toBeDefined();
    const footer = screen.getByRole('contentinfo');
    expect(
      within(footer).getByRole('button', { name: 'Nenhuma tarefa em andamento' }),
    ).toBeDefined();
    expect(await within(footer).findByText('Warden 0.1.0')).toBeDefined();
  });

  it('o primeiro Tab chega em "Pular para o conteúdo", que leva o foco ao <main>', async () => {
    mockBackend();
    renderApp('/');
    const skip = await screen.findByRole('link', { name: 'Pular para o conteúdo' });
    skip.focus();
    skip.click();
    expect(document.activeElement).toBe(screen.getByRole('main'));
    expect(await screen.findByText('Warden 0.1.0')).toBeDefined();
  });

  it('rota inexistente mostra a página em pt-BR com o caminho de volta', async () => {
    mockBackend();
    const { container } = renderApp('/nao-existe');
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Página não encontrada' }),
    ).toBeDefined();
    const back = screen.getByRole('link', { name: 'Voltar para o início' });
    expect(back.getAttribute('href')).toBe('/');
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('passa no axe (tela inteira, com landmarks)', async () => {
    mockBackend();
    const { container } = renderApp('/');
    await screen.findByText('Você ainda não tem packs');
    expect(await axePage(container)).toHaveNoViolations();
  });
});
