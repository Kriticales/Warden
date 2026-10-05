import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axePage } from '../test/axe';
import { ipcError, mockBackend } from '../test/backend';
import { makeAppError, makeAppInfo } from '../test/factories';
import { renderApp } from '../test/render';

describe('página inicial', () => {
  it('mostra a moldura do app: barra, conteúdo e rodapé com Tarefas', async () => {
    mockBackend();
    renderApp('/');

    const header = await screen.findByRole('banner');
    expect(within(header).getByText('Warden')).toBeDefined();
    const main = screen.getByRole('main');
    expect(main.id).toBe('conteudo');
    expect(
      within(main).getByRole('heading', { level: 1, name: 'Boas-vindas ao Warden' }),
    ).toBeDefined();
    expect(
      within(main).getByText('Crie, teste e diagnostique modpacks no formato packwiz.'),
    ).toBeDefined();
    const footer = screen.getByRole('contentinfo');
    expect(
      within(footer).getByRole('button', { name: 'Nenhuma tarefa em andamento' }),
    ).toBeDefined();
    expect(await within(footer).findByText('Warden 0.1.0')).toBeDefined();
  });

  it('F0-01 critério 2 e CA-T01-04: versão de app_info e o aviso legal em "Sobre o Warden"', async () => {
    mockBackend({ app_info: () => makeAppInfo({ version: '0.1.0', commit: 'abc123def456' }) });
    renderApp('/');

    const about = await screen.findByRole('region', { name: 'Sobre o Warden' });
    expect(await within(about).findByText('abc123def456')).toBeDefined();
    expect(within(about).getByText('0.1.0')).toBeDefined();
    expect(
      within(about).getByText(
        'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.',
      ),
    ).toBeDefined();
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

  it('mostra o erro de app_info com "Tentar de novo" sem derrubar a tela', async () => {
    mockBackend({
      app_info: () => ipcError(makeAppError({ domain: 'core', code: 'TIMEOUT' })),
    });
    renderApp('/');
    expect(
      await screen.findByText('A operação demorou demais e foi interrompida. Tente de novo.'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Tentar de novo' })).toBeDefined();
    expect(within(screen.getByRole('banner')).getByText('Warden')).toBeDefined();
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
    await screen.findByText('abc123def456');
    expect(await axePage(container)).toHaveNoViolations();
  });
});
