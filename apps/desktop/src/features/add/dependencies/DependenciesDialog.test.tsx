/**
 * Diálogo "Adicionar itens" (SPEC T09; esboço `design/bug-01/adicionar-itens.html`, opção 4 ·
 * Árvore + detalhe): um diálogo para vários itens com a biblioteca comum uma vez em "Em comum"
 * (CA-T09-05), o que só um mod pede pendurado nele, a mesma conta no título, no botão e no
 * rodapé, obrigatória desmarcada com aviso, o que pede decisão numa linha no topo (incompatível,
 * já no pack por outra loja; CA-T09-02, CA-T09-04), o aviso de lado uma vez e o porquê do item
 * clicado à direita.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { AddPlan, AddResult } from '../../../lib/ipc/bindings';
import { axeComponent } from '../../../test/axe';
import { ipcError, mockBackend } from '../../../test/backend';
import { makeAppError, nextPackId } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import { makeNode, makePlan } from '../add.fixtures';
import { DependenciesDialog, type DependenciesRequest } from './DependenciesDialog';

const TWO: DependenciesRequest = {
  choices: [
    { source: 'modrinth', projectId: 'mOgUt4GM', versionId: null },
    { source: 'modrinth', projectId: 'EsAfCjCV', versionId: null },
  ],
  titles: ['Mod Menu', 'AppleSkin'],
};

const ADDED: AddResult = {
  added: [
    { key: 'modrinth:mOgUt4GM', title: 'Mod Menu', path: 'mods/modmenu.pw.toml' },
    { key: 'modrinth:EsAfCjCV', title: 'AppleSkin', path: 'mods/appleskin.pw.toml' },
    {
      key: 'modrinth:eXts2L7r',
      title: 'Text Placeholder API',
      path: 'mods/placeholder-api.pw.toml',
    },
  ],
  skipped: [],
  removed: [],
};

function setup(plan: AddPlan, request: DependenciesRequest = TWO, apply: unknown = ADDED) {
  const backend = mockBackend({
    add_plan: () => plan,
    add_apply: () => (typeof apply === 'function' ? (apply as () => unknown)() : apply),
  });
  const onAdded = vi.fn();
  const onClose = vi.fn();
  renderWithProviders(
    <DependenciesDialog
      packId={nextPackId()}
      request={request}
      target={{ minecraft: '1.21.1', loader: 'fabric' }}
      onClose={onClose}
      onAdded={onAdded}
    />,
  );
  return { backend, onAdded, onClose };
}

function treeOf(dialog: HTMLElement) {
  return within(dialog).getByRole('table', { name: 'O que entra, por mod escolhido' });
}

/** A linha da árvore pelo nome do item. */
function rowOf(dialog: HTMLElement, name: string): HTMLElement {
  const button = within(treeOf(dialog)).getByRole('button', { name });
  const row = button.closest<HTMLElement>('[role="row"]');
  if (!row) throw new Error(`sem linha para ${name}`);
  return row;
}

/** O bloco (galho) da árvore em que a linha está. */
function branchOf(row: HTMLElement): HTMLElement {
  const branch = row.closest<HTMLElement>('[role="rowgroup"]');
  if (!branch) throw new Error('linha fora de um galho');
  return branch;
}

function detailOf(dialog: HTMLElement) {
  return within(dialog).getByRole('complementary', { name: 'Por que entra' });
}

/** O rodapé com a conta (o texto da `.conta`, com os espaços normalizados). */
function sumOf(dialog: HTMLElement): string {
  return (
    dialog.ownerDocument.querySelector('.conta')?.textContent.replace(/\s+/g, ' ').trim() ?? ''
  );
}

describe('diálogo Adicionar itens', () => {
  it('CA-T08-11/CA-T09-05: árvore por mod, a biblioteca comum uma vez em "Em comum", a mesma conta em todo lugar; axe', async () => {
    const { backend, onAdded } = setup(makePlan());
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 4 itens ao pack' });
    expect(
      within(dialog).getByText(
        'Nada foi gravado ainda. Confira o que entra no pack (1.21.1 · Fabric).',
      ),
    ).toBeDefined();
    // A Fabric API, pedida pelos dois, aparece uma vez, em "Em comum", com quem a pede.
    expect(within(treeOf(dialog)).getAllByRole('button', { name: 'Fabric API' })).toHaveLength(1);
    const fabric = rowOf(dialog, 'Fabric API');
    expect(branchOf(fabric).textContent).toContain('Em comum');
    expect(fabric.textContent).toContain('Pedida por Mod Menu e AppleSkin');
    // A Text Placeholder API, só do Mod Menu, fica pendurada no galho dele.
    expect(branchOf(rowOf(dialog, 'Text Placeholder API'))).toBe(
      branchOf(rowOf(dialog, 'Mod Menu')),
    );
    expect(rowOf(dialog, 'Text Placeholder API').textContent).toContain('obrigatória');
    // A mesma conta no título, no botão, no rodapé e nos blocos.
    expect(sumOf(dialog)).toBe('2 escolhidos + 2 obrigatórias = 4 itens');
    expect(within(dialog).getByRole('button', { name: 'Adicionar 4 itens' })).toBeDefined();
    expect(detailOf(dialog).textContent).toContain('4 itens vão entrar no pack');
    expect(
      within(dialog).getByText(
        'Nenhuma incompatibilidade declarada com os 4 itens que já estão no pack.',
      ),
    ).toBeDefined();
    expect(await axeComponent(dialog)).toHaveNoViolations();

    // Desmarcar uma obrigatória: o selo, a linha no topo e a conta mudam juntos.
    await userEvent.click(
      within(rowOf(dialog, 'Fabric API')).getByRole('checkbox', { name: 'Adicionar Fabric API' }),
    );
    expect(within(rowOf(dialog, 'Fabric API')).getByText('Jogo pode não abrir')).toBeDefined();
    expect(within(dialog).getByText('1 item precisa de você antes de adicionar')).toBeDefined();
    expect(screen.getByRole('dialog', { name: 'Adicionar 3 itens ao pack' })).toBeDefined();
    expect(sumOf(dialog)).toBe('2 escolhidos + 1 obrigatória = 3 itens');
    // Clicar no nome na linha do topo abre o detalhe com o botão que resolve.
    await userEvent.click(
      within(dialog.querySelector<HTMLElement>('.pb') ?? dialog).getByRole('button', {
        name: 'Fabric API',
      }),
    );
    expect(
      within(detailOf(dialog)).getByText('Sem Fabric API, o jogo provavelmente não abre'),
    ).toBeDefined();
    expect(
      within(detailOf(dialog)).getByText('Mod Menu e AppleSkin precisam dela. Você desmarcou.'),
    ).toBeDefined();
    expect(within(detailOf(dialog)).getByRole('button', { name: 'Marcar de novo' })).toBeDefined();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Adicionar 3 itens' }));

    await waitFor(() => {
      expect(onAdded).toHaveBeenCalledWith(ADDED);
    });
    const calls = backend.callsOf('add_apply');
    expect(calls).toHaveLength(1);
    expect(calls[0]?.args.request).toEqual({
      items: [
        {
          source: 'modrinth',
          projectId: 'mOgUt4GM',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'EsAfCjCV',
          versionId: 'V-APPLESKIN',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'eXts2L7r',
          versionId: 'V-PLACEHOLDER',
          side: 'client',
          replaces: null,
        },
      ],
    });
    expect(await screen.findByText('3 itens adicionados ao pack')).toBeDefined();
    expect(screen.getByText('Mod Menu, AppleSkin e Text Placeholder API.')).toBeDefined();
  });

  it('CA-T09-02/CA-T09-04: "2 itens precisam de você" (incompatível e já no pack por outra loja), sem versão e no pack', async () => {
    const plan = makePlan({
      nodes: [
        makeNode({ projectId: 'sk9rgfiA', slug: 'embeddium', title: 'Embeddium' }),
        makeNode({ projectId: 'mOgUt4GM', title: 'Mod Menu', versionId: 'V-MODMENU' }),
      ],
      installed: [
        {
          key: 'modrinth:P7dR8mSH',
          title: 'Fabric API',
          path: 'mods/fabric-api.pw.toml',
          packSource: 'curseforge',
          chosen: false,
          requiredBy: ['modrinth:mOgUt4GM'],
        },
      ],
      missing: [
        {
          key: 'modrinth:eXts2L7r',
          title: 'Text Placeholder API',
          role: 'required',
          requiredBy: ['modrinth:mOgUt4GM'],
        },
      ],
      conflicts: [
        {
          item: { key: 'modrinth:sk9rgfiA', title: 'Embeddium', path: null },
          other: { key: 'modrinth:AANobbMI', title: 'Sodium', path: 'mods/sodium.pw.toml' },
          withPack: true,
          declaredBy: 'modrinth:AANobbMI',
          reason: null,
        },
        {
          item: { key: 'modrinth:sk9rgfiA', title: 'Embeddium', path: null },
          other: { key: 'modrinth:mOgUt4GM', title: 'Mod Menu', path: null },
          withPack: false,
          declaredBy: 'modrinth:sk9rgfiA',
          reason: 'Os dois mexem no mesmo menu.',
        },
      ],
      duplicates: [
        {
          key: 'modrinth:mOgUt4GM',
          existingPath: 'mods/modmenu.pw.toml',
          existingTitle: 'Mod Menu (CurseForge)',
          existingSource: 'curseforge',
          matchedBy: 'hash',
        },
      ],
    });
    const { backend } = setup(plan, {
      choices: [
        { source: 'modrinth', projectId: 'sk9rgfiA', versionId: null },
        { source: 'modrinth', projectId: 'mOgUt4GM', versionId: null },
      ],
      titles: ['Embeddium', 'Mod Menu'],
    });
    // O Mod Menu já está no pack pela CurseForge e não entra por padrão: só o Embeddium.
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 1 item ao pack' });
    const attention = within(dialog).getByText('2 itens precisam de você antes de adicionar');
    const line = attention.closest<HTMLElement>('.pb');
    expect(line?.textContent).toContain('Embeddium incompatível com Sodium');
    expect(line?.textContent).toContain('Mod Menu já está no pack pela CurseForge');
    expect(within(rowOf(dialog, 'Embeddium')).getByText('Incompatível')).toBeDefined();
    expect(within(rowOf(dialog, 'Mod Menu')).getByText('Já no pack')).toBeDefined();
    // O que já está no pack (por outra loja) e o que não tem versão ficam no galho do Mod Menu.
    expect(within(rowOf(dialog, 'Fabric API')).getByText('no pack')).toBeDefined();
    const missing = rowOf(dialog, 'Text Placeholder API');
    expect(within(missing).getByText('Sem versão')).toBeDefined();
    expect(within(missing).getByRole<HTMLInputElement>('checkbox').disabled).toBe(true);
    // O detalhe abre no primeiro problema, com o motivo e quem declarou.
    const detail = detailOf(dialog);
    expect(
      within(detail).getByText('Embeddium é incompatível com Sodium, que já está no pack'),
    ).toBeDefined();
    expect(within(detail).getByText('Declarado por Sodium.')).toBeDefined();
    expect(within(detail).getByRole('button', { name: 'Não adicionar Embeddium' })).toBeDefined();

    // Mod Menu: escolher trocar pela versão do Modrinth.
    await userEvent.click(within(line ?? dialog).getByRole('button', { name: 'Mod Menu' }));
    expect(
      within(detailOf(dialog)).getByText(
        'mods/modmenu.pw.toml. Manter os dois faz o jogo travar com mod duplicado.',
      ),
    ).toBeDefined();
    await userEvent.click(
      within(detailOf(dialog)).getByRole('radio', { name: 'Trocar pela versão do Modrinth' }),
    );
    // Agora os dois entram, e a incompatibilidade entre eles também pede decisão.
    expect(screen.getByRole('dialog', { name: 'Adicionar 2 itens ao pack' })).toBeDefined();
    expect(within(rowOf(dialog, 'Mod Menu')).getByText('Substitui')).toBeDefined();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Adicionar 2 itens' }));
    await waitFor(() => {
      expect(backend.callsOf('add_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('add_apply')[0]?.args.request).toEqual({
      items: [
        {
          source: 'modrinth',
          projectId: 'sk9rgfiA',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'mOgUt4GM',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: 'mods/modmenu.pw.toml',
        },
      ],
    });
  });

  it('"Não adicionar" resolve pelo detalhe; "Adicionar mesmo assim" desfaz', async () => {
    const plan = makePlan({
      nodes: [makeNode({ projectId: 'sk9rgfiA', slug: 'embeddium', title: 'Embeddium' })],
      conflicts: [
        {
          item: { key: 'modrinth:sk9rgfiA', title: 'Embeddium', path: null },
          other: { key: 'modrinth:AANobbMI', title: 'Sodium', path: 'mods/sodium.pw.toml' },
          withPack: true,
          declaredBy: 'modrinth:sk9rgfiA',
          reason: 'os dois trocam o mesmo motor de desenho do jogo; use só um.',
        },
      ],
    });
    setup(plan, {
      choices: [{ source: 'modrinth', projectId: 'sk9rgfiA', versionId: null }],
      titles: ['Embeddium'],
    });
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 1 item ao pack' });
    await userEvent.click(
      within(detailOf(dialog)).getByRole('button', { name: 'Não adicionar Embeddium' }),
    );
    expect(screen.getByRole('dialog', { name: 'Nada marcado para adicionar' })).toBeDefined();
    expect(within(detailOf(dialog)).getByText('Embeddium não vai entrar')).toBeDefined();
    expect(within(rowOf(dialog, 'Embeddium')).getByText('Desmarcado')).toBeDefined();
    expect(sumOf(dialog)).toBe('Marque pelo menos um item.');
    expect(
      within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Adicionar 0 itens' }).disabled,
    ).toBe(true);
    await userEvent.click(
      within(detailOf(dialog)).getByRole('button', { name: 'Adicionar mesmo assim' }),
    );
    expect(screen.getByRole('dialog', { name: 'Adicionar 1 item ao pack' })).toBeDefined();
  });

  it('B1-B: título, botão e rodapé fazem a mesma conta, e "Pedida por" só cita quem entra', async () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes,
        makeNode({
          projectId: 'nU0bVIaL',
          slug: 'badpackets',
          title: 'Bad Packets',
          role: 'optional',
          versionId: 'V-BADPACKETS',
          optionalFor: ['modrinth:EsAfCjCV'],
        }),
      ],
    });
    setup(plan);
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 4 itens ao pack' });
    expect(within(dialog).getByRole('button', { name: 'Adicionar 4 itens' })).toBeDefined();
    // A opcional fica pendurada no AppleSkin, desmarcada.
    const bad = rowOf(dialog, 'Bad Packets');
    expect(branchOf(bad)).toBe(branchOf(rowOf(dialog, 'AppleSkin')));
    await userEvent.click(within(bad).getByRole('checkbox', { name: 'Adicionar Bad Packets' }));
    expect(screen.getByRole('dialog', { name: 'Adicionar 5 itens ao pack' })).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar 5 itens' })).toBeDefined();
    expect(sumOf(dialog)).toBe('2 escolhidos + 2 obrigatórias + 1 opcional = 5 itens');
    await userEvent.click(within(bad).getByRole('checkbox', { name: 'Adicionar Bad Packets' }));

    // Desmarcar o AppleSkin apaga o galho dele: a Fabric API só é pedida pelo Mod Menu e a
    // opcional dele não entra (selo "Não entra").
    await userEvent.click(
      within(rowOf(dialog, 'AppleSkin')).getByRole('checkbox', { name: 'Adicionar AppleSkin' }),
    );
    expect(branchOf(rowOf(dialog, 'AppleSkin')).classList.contains('is-off')).toBe(true);
    expect(rowOf(dialog, 'Fabric API').textContent).toContain('Pedida por Mod Menu');
    expect(rowOf(dialog, 'Fabric API').textContent).not.toContain('AppleSkin');
    const offBad = within(rowOf(dialog, 'Bad Packets'));
    expect(offBad.getByText('Não entra')).toBeDefined();
    expect(offBad.getByRole<HTMLInputElement>('checkbox').disabled).toBe(true);
    expect(screen.getByRole('dialog', { name: 'Adicionar 3 itens ao pack' })).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar 3 itens' })).toBeDefined();
    expect(sumOf(dialog)).toBe('1 escolhido + 2 obrigatórias = 3 itens');
  });

  it('o porquê do item clicado: o que faz, versão, arquivo, fonte e lado; o aviso de lado aparece uma vez', async () => {
    const plan = makePlan({
      nodes: [
        makeNode({ sideNote: 'unknown', side: 'both' }),
        makeNode({
          projectId: 'EsAfCjCV',
          title: 'AppleSkin',
          summary: 'Mostra a fome e a saturação.',
          versionNumber: '3.0.6',
          fileName: 'appleskin-fabric-mc1.21-3.0.6.jar',
          source: 'curseforge',
          key: 'curseforge:248787',
          sideNote: 'unknown',
          side: 'both',
        }),
      ],
    });
    setup(plan);
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 2 itens ao pack' });
    expect(
      within(dialog).getAllByText(
        (_, element) =>
          element?.classList.contains('deps-note') === true &&
          element.textContent.startsWith('Lado não informado em 2 itens: Mod Menu e AppleSkin.'),
      ),
    ).toHaveLength(1);
    await userEvent.click(within(treeOf(dialog)).getByRole('button', { name: 'AppleSkin' }));
    const detail = detailOf(dialog);
    expect(within(detail).getByRole('heading', { name: 'AppleSkin' })).toBeDefined();
    expect(within(detail).getByText('Mostra a fome e a saturação.')).toBeDefined();
    expect(within(detail).getByText('Vai entrar')).toBeDefined();
    expect(within(detail).getByText('3.0.6')).toBeDefined();
    expect(within(detail).getByText('Estável · a mais nova para 1.21.1 · Fabric')).toBeDefined();
    expect(within(detail).getByText('appleskin-fabric-mc1.21-3.0.6.jar')).toBeDefined();
    expect(within(detail).getByText('CurseForge')).toBeDefined();
    expect(within(detail).getByText('Não informado')).toBeDefined();
    // Os blocos da conta também abrem o detalhe.
    await userEvent.click(within(detail).getByRole('button', { name: 'Mod Menu' }));
    expect(within(detailOf(dialog)).getByRole('heading', { name: 'Mod Menu' })).toBeDefined();
  });

  it('falha ao gravar: o erro aparece no diálogo e nada é dado como adicionado', async () => {
    const { onAdded } = setup(makePlan(), TWO, () =>
      ipcError(
        makeAppError(
          { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
          {
            params: { source: 'Modrinth' },
          },
        ),
      ),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 4 itens ao pack' });
    await userEvent.click(await within(dialog).findByRole('button', { name: 'Adicionar 4 itens' }));
    expect(
      await within(dialog).findByText('Não foi possível adicionar. Nada foi gravado.'),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'Não foi possível falar com o Modrinth agora. Confira a internet e tente de novo.',
      ),
    ).toBeDefined();
    expect(onAdded).not.toHaveBeenCalled();
  });
});
