/**
 * A pré-seleção da etapa Loader (T03, CA-T03-09) sem o assistente em volta: o recomendado já
 * aparece marcado quando as listas chegam, antes de a pré-seleção ser gravada, para as opções de
 * baixo não pularem de lugar logo depois de aparecer (achado do E2E `full-flow` no Linux); e a
 * gravação da pré-seleção nunca desfaz a escolha do usuário (`preselectLoader`).
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { Loader } from '../../../../lib/ipc/bindings';
import { mockBackend } from '../../../../test/backend';
import { makeLoaderVersions } from '../../../../test/factories';
import { renderWithProviders } from '../../../../test/render';
import { preselectLoader, type Draft } from './draft';
import { LoaderStep } from './LoaderStep';

const VERSIONS: Partial<Record<Loader, string[]>> = {
  neoforge: ['21.1.252'],
  forge: ['52.0.1'],
  fabric: ['0.16.5'],
};

function radioOf(title: string): HTMLInputElement {
  const group = screen.getByRole('group', { name: 'Loader' });
  const card = within(group)
    .getAllByText(title, { selector: '.choice__title' })
    .map((element) => element.closest('.choice'))
    .find(Boolean);
  const radio = card?.querySelector<HTMLInputElement>('input[type="radio"]');
  if (!radio) throw new Error(`sem o rádio de ${title}`);
  return radio;
}

describe('etapa Loader: pré-seleção (CA-T03-09)', () => {
  it('o recomendado já aparece marcado, com "Escolher outra versão", antes de a pré-seleção ser gravada', async () => {
    mockBackend({
      catalog_loader_versions: ({ loader, minecraft }) =>
        makeLoaderVersions(loader as Loader, minecraft as string, VERSIONS[loader as Loader] ?? []),
    });
    const onPreselect = vi.fn();
    // O pai não grava nada: a tela mostra a pré-seleção sozinha, na renderização em que as
    // listas chegam.
    renderWithProviders(
      <LoaderStep
        minecraft="1.21.1"
        loader={null}
        version={null}
        ready={false}
        onChange={vi.fn()}
        onPreselect={onPreselect}
      />,
    );
    await screen.findByText('Fabric', { selector: '.choice__title' }, { timeout: 10_000 });
    expect(radioOf('NeoForge').checked).toBe(true);
    expect(radioOf('Fabric').checked).toBe(false);
    expect(screen.getByRole('button', { name: 'Escolher outra versão do NeoForge' })).toBeDefined();
    await waitFor(() => {
      expect(onPreselect).toHaveBeenCalledWith('neoforge', '21.1.252');
    });
  });
});

describe('preselectLoader', () => {
  const draft: Draft = {
    name: 'Vale do Fluxo',
    author: '',
    description: '',
    destination: null,
    minecraft: '1.21.1',
    loader: null,
    loaderVersion: null,
    loaderReady: false,
    initial: null,
  };

  it('grava o recomendado enquanto ninguém escolheu', () => {
    expect(preselectLoader(draft, 'neoforge', '21.1.252')).toEqual({
      ...draft,
      loader: 'neoforge',
      loaderVersion: '21.1.252',
      loaderReady: true,
    });
  });

  it('não desfaz a escolha que chegou antes do efeito da pré-seleção', () => {
    const chosen: Draft = {
      ...draft,
      loader: 'fabric',
      loaderVersion: '0.16.5',
      loaderReady: true,
    };
    expect(preselectLoader(chosen, 'neoforge', '21.1.252')).toBe(chosen);
  });
});
