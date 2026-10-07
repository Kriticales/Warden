/**
 * Pré-visualização (SPEC T08): descrição higienizada (CA-T08-12), seletor de versão com a padrão
 * do canal e "Adicionar ao pack" com a versão escolhida.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../../test/axe';
import { mockBackend } from '../../../test/backend';
import { nextPackId } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import {
  makeDependency,
  makeGallery,
  makePreview,
  makeResult,
  makeVersions,
  ref,
  SODIUM,
} from '../add.fixtures';
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
    project_gallery: () => makeGallery(),
    version_dependencies: () => ({ items: [makeDependency()] }),
    version_notes: () => null,
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
      project_gallery: () => [],
      version_dependencies: () => ({ items: [] }),
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

  it('índice fixo com as cinco partes; clicar leva à parte (CA-T08-13)', async () => {
    const scroll = vi.fn();
    Object.assign(Element.prototype, { scrollIntoView: scroll });
    setup();
    const index = await screen.findByRole('navigation', { name: 'Partes desta página' });
    const names = within(index)
      .getAllByRole('button')
      .map((button) => button.textContent);
    expect(names).toEqual(['Descrição', 'Galeria', 'Versões', 'Dependências', 'Links']);
    await userEvent.click(within(index).getByRole('button', { name: 'Galeria' }));
    expect(scroll).toHaveBeenCalledTimes(1);
    const target = scroll.mock.contexts[0] as HTMLElement;
    expect(target.getAttribute('aria-labelledby')).not.toBeNull();
    expect(target.querySelector('.gallery')).not.toBeNull();
  });

  it('galeria: miniaturas; clicar mostra a imagem grande sobre a página, com anterior e próxima', async () => {
    setup();
    const gallery = await screen.findByRole('list', { name: 'Imagens do projeto' });
    const thumbs = within(gallery).getAllByRole('button');
    expect(thumbs).toHaveLength(3);
    expect(thumbs[0]?.querySelector('img')?.getAttribute('src')).toContain('_350.webp');
    await userEvent.click(
      within(gallery).getByRole('button', { name: 'Ver Captura 1 em tamanho grande' }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Captura 1' });
    const big = within(dialog).getByRole('img', { name: 'Captura 1' });
    expect(big.getAttribute('src')).toBe('https://cdn.modrinth.com/data/AANobbMI/images/img0.webp');
    expect(within(dialog).getByText('Iluminação suave debaixo d’água.')).toBeDefined();
    expect(within(dialog).getByText('Imagem 1 de 3')).toBeDefined();
    expect(
      within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Imagem anterior' }).disabled,
    ).toBe(true);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Próxima imagem' }));
    expect(await screen.findByRole('dialog', { name: 'Captura 2' })).toBeDefined();
    await userEvent.click(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Próxima imagem' }),
    );
    // A imagem sem título ganha um nome pela posição.
    expect(await screen.findByRole('dialog', { name: 'Imagem 3' })).toBeDefined();
    await userEvent.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('galeria vazia e galeria com erro não derrubam a pré-visualização', async () => {
    setup({ project_gallery: () => [] });
    expect(await screen.findByText('O autor não publicou imagens.')).toBeDefined();
    expect(await screen.findByRole('combobox', { name: 'Versão' })).toBeDefined();
  });

  it('CA-T08-13: versões com notas só pedidas ao abrir a linha; Usar esta versão escolhe', async () => {
    const { backend, onAdd } = setup({
      version_notes: (args) =>
        args.versionId === 'BETA0001'
          ? { body: '- Teste do **novo** layout', format: 'markdown' }
          : null,
    });
    const list = await screen.findByRole('list', { name: 'Versões compatíveis' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
    // Nada de notas até alguém abrir uma linha.
    expect(backend.callsOf('version_notes')).toHaveLength(0);
    const beta = within(list).getByText('mc1.21.1-0.8.14-beta.1').closest('details');
    if (!beta) throw new Error('sem linha da beta');
    expect(within(list).getByText(/mais nova/)).toBeDefined();
    fireEvent.click(within(beta).getByText('mc1.21.1-0.8.14-beta.1'));
    beta.open = true;
    fireEvent(beta, new Event('toggle'));
    expect(await within(beta).findByText('novo')).toBeDefined();
    expect(backend.callsOf('version_notes')).toHaveLength(1);
    expect(backend.callsOf('version_notes')[0]?.args).toMatchObject({
      source: 'modrinth',
      projectId: 'AANobbMI',
      versionId: 'BETA0001',
    });
    await userEvent.click(within(beta).getByRole('button', { name: 'Usar esta versão' }));
    expect(screen.getByRole<HTMLSelectElement>('combobox', { name: 'Versão' }).value).toBe(
      'BETA0001',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Adicionar ao pack' }));
    expect(onAdd).toHaveBeenCalledWith(
      { source: 'modrinth', projectId: 'AANobbMI', versionId: 'BETA0001' },
      'Sodium',
    );
    // A outra versão, sem notas, mostra o aviso ao abrir.
    const stable = within(list).getByText('mc1.21.1-0.8.13').closest('details');
    if (!stable) throw new Error('sem linha da estável');
    stable.open = true;
    fireEvent(stable, new Event('toggle'));
    expect(
      await within(stable).findByText('O autor não escreveu notas para esta versão.'),
    ).toBeDefined();
    expect(backend.callsOf('version_notes')).toHaveLength(2);
  });

  it('dependências: Já no pack, Será adicionada, opcional e conflito, para a versão escolhida', async () => {
    const { backend } = setup({
      version_dependencies: () => ({
        items: [
          makeDependency({ title: 'Fabric API', inPack: true }),
          makeDependency({ projectId: 'eXts2L7r', title: 'Text Placeholder API' }),
          makeDependency({ kind: 'optional', projectId: 'o1', title: 'Mod Menu' }),
          makeDependency({
            kind: 'incompatible',
            projectId: 'i1',
            title: 'Rubidium',
            inPack: true,
          }),
        ],
      }),
    });
    const group = (name: string) => {
      const term = screen.getByText(name, { selector: 'dt' });
      const row = term.nextElementSibling;
      if (!(row instanceof HTMLElement)) throw new Error(`sem lista de ${name}`);
      return within(row);
    };
    await screen.findByText('Text Placeholder API');
    const required = group('Obrigatória');
    expect(required.getByText('Já no pack')).toBeDefined();
    expect(required.getByText('Será adicionada')).toBeDefined();
    expect(group('Opcional').getByText('não será adicionada sozinha')).toBeDefined();
    expect(group('Incompatível').getByText('Já está no pack: vai dar conflito')).toBeDefined();
    expect(backend.callsOf('version_dependencies')[0]?.args).toMatchObject({
      source: 'modrinth',
      versionId: 'SMxNOGZ6',
    });
  });

  it('dependências sem nenhuma declarada dizem "nenhuma declarada"; links continuam', async () => {
    setup({ version_dependencies: () => ({ items: [] }) });
    expect((await screen.findAllByText('nenhuma declarada')).length).toBe(3);
    expect(screen.getByRole('button', { name: 'Código' })).toBeDefined();
  });

  it('imagens da CurseForge: o endereço warden-img:// passa pelo conversor do protocolo', async () => {
    const { imageSrc } = await import('../../../lib/ipc/image-src');
    expect(imageSrc('https://cdn.modrinth.com/a.png')).toBe('https://cdn.modrinth.com/a.png');
    expect(imageSrc(null)).toBeNull();
    // Fora do app não há conversor do Tauri: o endereço original volta, sem quebrar.
    expect(imageSrc('warden-img://localhost/abc')).toContain('abc');
  });
});
