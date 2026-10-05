import { fireEvent, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../test/axe';
import { mockBackend } from '../../test/backend';
import { renderWithProviders } from '../../test/render';
import { SafeHtml } from './SafeHtml';
import { SafeMarkdown } from './SafeMarkdown';

const MODRINTH_BODY = `# Just Enough Items

<p align="center"><img src="https://cdn.modrinth.com/jei.png" alt="logo"></p>

Mostra **receitas** e [usos](https://modrinth.com/mod/jei) dos itens.

| Versão | Loader |
| --- | --- |
| 1.20.1 | Forge |

<script>alert('xss')</script>
<a href="javascript:alert(1)">perigo</a>
<img src="x" onerror="alert(1)">

<iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ"></iframe>

<details><summary>Changelog</summary>Correções.</details>
`;

describe('SafeMarkdown', () => {
  it('mistura de Markdown e HTML do Modrinth: estrutura fica, o perigoso sai', async () => {
    mockBackend();
    const { container } = renderWithProviders(<SafeMarkdown>{MODRINTH_BODY}</SafeMarkdown>);
    expect(screen.getByRole('heading', { level: 1, name: 'Just Enough Items' })).toBeDefined();
    expect(screen.getByRole('table')).toBeDefined();
    expect(screen.getByRole('img', { name: 'logo' })).toBeDefined();
    expect(screen.getByText('Changelog')).toBeDefined();
    expect(container.querySelector('script')).toBeNull();
    expect(container.innerHTML).not.toMatch(/javascript:|onerror|<iframe/i);
    // O link sem https vira texto.
    expect(screen.getByText('perigo').tagName).toBe('SPAN');
    // A imagem sem endereço válido perde o src.
    expect(container.querySelector('img[src="x"]')).toBeNull();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('link https abre no navegador, sem navegar a janela', async () => {
    const backend = mockBackend();
    renderWithProviders(<SafeMarkdown>{'[JEI](https://modrinth.com/mod/jei)'}</SafeMarkdown>);
    fireEvent.click(screen.getByRole('link', { name: 'JEI' }));
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')[0]?.args.url).toBe(
        'https://modrinth.com/mod/jei',
      );
    });
  });

  it('vídeo vira miniatura com "Abrir no navegador"', async () => {
    const backend = mockBackend();
    renderWithProviders(
      <SafeMarkdown>
        {'<iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ"></iframe>'}
      </SafeMarkdown>,
    );
    expect(screen.getByRole('img', { name: 'Miniatura do vídeo' }).getAttribute('src')).toBe(
      'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
    );
    expect(screen.getByText('Vídeo de youtube.com')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Abrir no navegador' }));
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')).toHaveLength(1);
    });
  });
});

describe('SafeHtml', () => {
  it('HTML da CurseForge higienizado, com link e vídeo funcionando', async () => {
    const backend = mockBackend();
    const { container } = renderWithProviders(
      <SafeHtml
        html={`<h2>Sobre</h2><p onclick="alert(1)">Veja <a href="https://www.curseforge.com/x">a página</a>.</p><script>alert(1)</script><iframe src="https://vimeo.com/123"></iframe>`}
      />,
    );
    expect(screen.getByRole('heading', { level: 2, name: 'Sobre' })).toBeDefined();
    expect(container.querySelector('script, [onclick]')).toBeNull();
    fireEvent.click(screen.getByRole('link', { name: 'a página' }));
    // Vídeo sem miniatura conhecida: só o site e o botão.
    expect(await screen.findByText('Vídeo de vimeo.com')).toBeDefined();
    expect(screen.queryByRole('img')).toBeNull();
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')[0]?.args.url).toBe(
        'https://www.curseforge.com/x',
      );
    });
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('clique fora de link não abre nada', () => {
    const backend = mockBackend();
    renderWithProviders(<SafeHtml html="<p>texto</p>" />);
    fireEvent.click(screen.getByText('texto'));
    expect(backend.callsOf('plugin:opener|open_url')).toHaveLength(0);
  });
});
