/**
 * Pré-visualização (SPEC T08): descrição higienizada (CA-T08-12), seletor de versão com a padrão
 * do canal e "Adicionar ao pack" com a versão escolhida.
 */
import { fireEvent, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../../test/axe';
import { mockBackend } from '../../../test/backend';
import { nextPackId } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import { makePreview, makeResult, makeVersions, ref, SODIUM } from '../add.fixtures';
import { Preview } from './Preview';

/** Corpo do Modrinth com HTML misturado ao Markdown (CA-T08-12). */
const MIXED_BODY = `<h1 style="color:red;font-size:80px">Sodium</h1>

Motor de renderização **muito** mais rápido.

<iframe width="560" height="315" src="https://www.youtube.com/embed/dQw4w9WgXcQ"></iframe>

<img src="https://cdn.modrinth.com/x.png" alt="captura" onerror="alert(1)">

<script>window.hacked = true</script>

<p style="background:url(javascript:alert(1))">Texto com estilo</p>
`;

const TARGET = { minecraft: '1.21.1', loader: 'fabric' };

function setup(handlers: Parameters<typeof mockBackend>[0] = {}) {
  const backend = mockBackend({
    project_details: () => makePreview({ body: MIXED_BODY }),
    project_versions: () => makeVersions(),
    ...handlers,
  });
  const onAdd = vi.fn();
  const view = renderWithProviders(
    <Preview
      packId={nextPackId()}
      result={SODIUM}
      inPack={false}
      kind="mod"
      target={TARGET}
      onAdd={onAdd}
    />,
  );
  return { backend, onAdd, ...view };
}

describe('pré-visualização', () => {
  it('CA-T08-12: descrição com HTML aparece formatada, sem iframe, estilo inline nem script; vídeo vira miniatura', async () => {
    const { backend, container } = setup();
    const heading = await screen.findByRole('heading', { level: 1, name: 'Sodium' });
    expect(heading.hasAttribute('style')).toBe(false);
    const description = heading.closest('.prose');
    expect(description).not.toBeNull();
    expect(screen.getByText('muito').tagName).toBe('STRONG');
    expect(container.querySelector('iframe')).toBeNull();
    expect(container.querySelector('script')).toBeNull();
    expect(description?.querySelector('[style]')).toBeNull();
    expect(container.innerHTML).not.toMatch(/onerror|javascript:|window\.hacked/i);
    expect(screen.getByRole('img', { name: 'captura' })).toBeDefined();
    expect(screen.getByRole('img', { name: 'Miniatura do vídeo' })).toBeDefined();
    expect((window as { hacked?: boolean }).hacked).toBeUndefined();
    fireEvent.click(screen.getByRole('button', { name: 'Abrir no navegador' }));
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')[0]?.args.url).toBe(
        'https://www.youtube.com/embed/dQw4w9WgXcQ',
      );
    });
  });

  it('versão padrão é a estável mais nova; Adicionar leva a versão escolhida; axe', async () => {
    // Títulos da descrição são do autor (podem pular níveis); aqui o corpo é só texto.
    const { onAdd, container } = setup({
      project_details: () => makePreview({ body: 'Deixa o jogo **mais leve**.' }),
    });
    const select = await screen.findByRole('combobox', { name: 'Versão' });
    expect((select as HTMLSelectElement).value).toBe('SMxNOGZ6');
    expect(
      screen.getByRole('option', { name: 'mc1.21.1-0.8.13 (mais nova compatível)' }),
    ).toBeDefined();
    expect(screen.getByRole('option', { name: 'mc1.21.1-0.8.14-beta.1 (beta)' })).toBeDefined();
    // Uma fonte só: sem seletor Fonte.
    expect(screen.queryByRole('combobox', { name: 'Fonte' })).toBeNull();
    await screen.findByText('mais leve');
    expect(await axeComponent(container)).toHaveNoViolations();
    await userEvent.selectOptions(select, 'BETA0001');
    await userEvent.click(screen.getByRole('button', { name: 'Adicionar ao pack' }));
    expect(onAdd).toHaveBeenCalledWith(
      { source: 'modrinth', projectId: 'AANobbMI', versionId: 'BETA0001' },
      'Sodium',
    );
  });

  it('sem versão para o pack: aviso e Adicionar indisponível', async () => {
    setup({ project_versions: () => makeVersions({ versions: [], defaultId: null }) });
    expect(await screen.findByText('Sem versão para Minecraft 1.21.1 com Fabric.')).toBeDefined();
    expect(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Adicionar ao pack' }).disabled,
    ).toBe(true);
  });

  it('projeto nas duas fontes: seletor Fonte com o Modrinth recomendado', async () => {
    mockBackend({
      project_details: () => makePreview(),
      project_versions: () => makeVersions(),
    });
    renderWithProviders(
      <Preview
        packId={nextPackId()}
        result={makeResult({
          sources: [ref('AANobbMI'), ref('394468', { source: 'curseforge' })],
        })}
        inPack
        kind="mod"
        target={TARGET}
        onAdd={vi.fn()}
      />,
    );
    const source = await screen.findByRole('combobox', { name: 'Fonte' });
    expect((source as HTMLSelectElement).value).toBe('modrinth');
    expect(screen.getByRole('option', { name: 'Modrinth (recomendada)' })).toBeDefined();
    expect(screen.getByText('Já no pack.')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Adicionar ao pack' })).toBeNull();
  });
});
