import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { ImportedPack, ImportPreview } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../../test/backend';
import {
  makeAppError,
  makeHygieneFinding,
  makeImportPreview,
  makePackRow,
} from '../../../test/factories';
import { renderApp } from '../../../test/render';

const FOLDER = 'D:/Packs/meu-pack-antigo';
const PACK_ID = '01JA0000000000000000000077';

function imported(overrides: Partial<ImportedPack> = {}): ImportedPack {
  return {
    id: PACK_ID,
    path: FOLDER,
    copiedIdReplaced: false,
    requiredIgnoreAdded: [],
    readOnlyReason: null,
    ...overrides,
  };
}

/** Os três itens do CA-T04-01. */
const DIRTY = [
  makeHygieneFinding({ path: 'config/x.toml.bak', bytes: 4096 }),
  makeHygieneFinding({
    path: 'packwiz-installer-bootstrap.jar',
    bytes: 59392,
    reasons: [{ kind: 'pattern', pattern: '/packwiz-installer*.jar', group: 'serverOnly' }],
  }),
  makeHygieneFinding({
    path: '.packwiz.toml',
    bytes: 512,
    reasons: [{ kind: 'pattern', pattern: '/.packwiz.toml', group: 'serverOnly' }],
  }),
];

function openBackend(preview: ImportPreview | Handler, extra: Record<string, Handler> = {}) {
  return mockBackend({
    pack_import_preview: typeof preview === 'function' ? preview : () => preview,
    pack_import: () => imported(),
    pack_hygiene_fix: ({ paths }) => paths,
    pack_get: () => makePackRow({ id: PACK_ID, name: 'Meu pack antigo', path: FOLDER }),
    pack_hygiene_scan: () => [],
    ...extra,
  });
}

function renderOpen() {
  return renderApp(`/packs/abrir?pasta=${encodeURIComponent(FOLDER)}`);
}

const WRITES = ['pack_import', 'pack_hygiene_fix', 'pack_create', 'pack_relocate'];

describe('Abrir pack (T04)', () => {
  it('mostra o pack, o loader suportado e a higiene com tamanho e motivo; passa no axe', async () => {
    const backend = openBackend(makeImportPreview({ hygiene: DIRTY }));
    const { container } = renderOpen();
    expect(await screen.findByText('Meu pack antigo', {}, { timeout: 5000 })).toBeDefined();
    expect(screen.getByText(`Pasta packwiz em ${FOLDER}`)).toBeDefined();
    expect(screen.getByText('Minecraft 1.20.1 · NeoForge')).toBeDefined();
    expect(screen.getByText('suportado')).toBeDefined();
    const panel = screen.getByRole('region', {
      name: '3 arquivos não deveriam ir para quem joga o pack',
    });
    const rows = within(panel).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.textContent)).toEqual([
      'config/x.toml.bakvai para os jogadores4,0 KBcópia de segurança',
      'packwiz-installer-bootstrap.jarvai para os jogadores58,0 KBferramenta, não conteúdo',
      '.packwiz.tomlvai para os jogadores512 Barquivo de outro programa',
    ]);
    for (const row of rows) {
      expect(within(row).getByRole<HTMLInputElement>('checkbox').checked).toBe(true);
    }
    expect(backend.callsOf('pack_import_preview')[0]?.args).toEqual({ path: FOLDER });
    // Verificar não escreve nada.
    expect(backend.calls.filter((call) => WRITES.includes(call.command))).toEqual([]);
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('CA-T04-01 (interface): "Limpar" registra o pack e limpa os três marcados', async () => {
    const user = userEvent.setup();
    const backend = openBackend(makeImportPreview({ hygiene: DIRTY }));
    renderOpen();
    await user.click(await screen.findByRole('button', { name: 'Limpar 3 arquivos e abrir' }));
    expect(
      await screen.findByText('3 arquivos limpos. O ponto de segurança está no Histórico.'),
    ).toBeDefined();
    expect(await screen.findByRole('heading', { level: 1, name: 'Meu pack antigo' })).toBeDefined();
    expect(backend.calls.filter((call) => WRITES.includes(call.command))).toEqual([
      { command: 'pack_import', args: { path: FOLDER, addControls: true } },
      {
        command: 'pack_hygiene_fix',
        args: {
          packId: PACK_ID,
          paths: ['config/x.toml.bak', 'packwiz-installer-bootstrap.jar', '.packwiz.toml'],
        },
      },
    ]);
  });

  it('desmarcar um item tira ele da limpeza', async () => {
    const user = userEvent.setup();
    const backend = openBackend(makeImportPreview({ hygiene: DIRTY }));
    renderOpen();
    await user.click(await screen.findByRole('checkbox', { name: 'Limpar .packwiz.toml' }));
    await user.click(screen.getByRole('button', { name: 'Limpar 2 arquivos e abrir' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_hygiene_fix')[0]?.args.paths).toEqual([
        'config/x.toml.bak',
        'packwiz-installer-bootstrap.jar',
      ]);
    });
  });

  it('CA-T04-04 (interface): "Agora não" só registra o pack, sem limpar', async () => {
    const user = userEvent.setup();
    const backend = openBackend(makeImportPreview({ hygiene: DIRTY }), {
      pack_import: () => imported({ requiredIgnoreAdded: ['/.warden/', '/CHANGELOG.md'] }),
    });
    renderOpen();
    await user.click(await screen.findByRole('button', { name: 'Agora não' }));
    expect(
      await screen.findByText('Linhas acrescentadas ao .packwizignore: /.warden/, /CHANGELOG.md'),
    ).toBeDefined();
    expect(await screen.findByRole('heading', { level: 1, name: 'Meu pack antigo' })).toBeDefined();
    expect(backend.calls.filter((call) => WRITES.includes(call.command))).toEqual([
      { command: 'pack_import', args: { path: FOLDER, addControls: true } },
    ]);
  });

  it('CA-T04-02 (interface): pack Quilt mostra a mensagem e não registra nada', async () => {
    const backend = openBackend(() =>
      ipcError(
        makeAppError(
          { domain: 'project', code: 'UNSUPPORTED_LOADER' },
          { params: { loader: 'quilt' } },
        ),
      ),
    );
    const { container } = renderOpen();
    expect(
      await screen.findByText('Este pack usa Quilt, que o Warden ainda não suporta.'),
    ).toBeDefined();
    expect(
      screen.getByText(
        'Nada foi alterado na pasta. Packs Forge, NeoForge e Fabric abrem normalmente.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Escolher outra pasta' })).toBeDefined();
    expect(backend.calls.filter((call) => WRITES.includes(call.command))).toEqual([]);
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('mais de um loader e pasta sem pack.toml têm frases próprias', async () => {
    openBackend(() =>
      ipcError(
        makeAppError(
          { domain: 'project', code: 'UNSUPPORTED_LOADER' },
          { params: { reason: 'multiple' } },
        ),
      ),
    );
    const first = renderOpen();
    expect(
      await screen.findByText('Este pack declara mais de um loader e não pode ser aberto.'),
    ).toBeDefined();
    first.unmount();

    openBackend(() =>
      ipcError(
        makeAppError(
          { domain: 'project', code: 'INVALID_PACK' },
          { detail: 'pack.toml: arquivo não encontrado' },
        ),
      ),
    );
    renderOpen();
    expect(
      await screen.findByText('Esta pasta não tem um pack packwiz que o Warden consiga ler.'),
    ).toBeDefined();
    expect(screen.getByText(/Importar \.mrpack, zip da CurseForge/)).toBeDefined();
    expect(screen.getByText('pack.toml: arquivo não encontrado')).toBeDefined();
  });

  it('arquivos de controle: mostra a diferença e respeita a escolha de não adicionar', async () => {
    const user = userEvent.setup();
    const backend = openBackend(
      makeImportPreview({
        missingControls: ['.gitattributes'],
        controlDiffs: [
          { path: '.gitattributes', current: null, suggested: '* -text\n' },
          { path: '.packwizignore', current: '*.log\n', suggested: '*.log\n/.warden/\n' },
        ],
      }),
    );
    renderOpen();
    const panel = await screen.findByRole('region', { name: 'Arquivos de controle' });
    expect(within(panel).getByText('falta')).toBeDefined();
    expect(within(panel).getByText('diferente do padrão')).toBeDefined();
    await user.click(within(panel).getByText('.packwizignore', { selector: 'summary span' }));
    expect(await within(panel).findByText('/.warden/')).toBeDefined();
    const toggle = within(panel).getByRole('checkbox', {
      name: 'Adicionar os arquivos de controle do Warden',
    });
    expect((toggle as HTMLInputElement).checked).toBe(true);
    await user.click(toggle);
    await user.click(screen.getByRole('button', { name: 'Abrir pack' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_import')[0]?.args).toEqual({ path: FOLDER, addControls: false });
    });
  });

  it('CA-T04-03 (interface): cópia de um pack registrado avisa do identificador novo', async () => {
    const user = userEvent.setup();
    openBackend(makeImportPreview(), {
      pack_import: () => imported({ copiedIdReplaced: true }),
    });
    renderOpen();
    await user.click(await screen.findByRole('button', { name: 'Abrir pack' }));
    expect(
      await screen.findByText(
        'Este pack é uma cópia de outro da lista e ganhou um identificador novo. Os dois têm testes separados.',
      ),
    ).toBeDefined();
  });

  it('git em estado só de leitura: avisa, não oferece limpar e abre só para leitura', async () => {
    const user = userEvent.setup();
    const backend = openBackend(
      makeImportPreview({ hygiene: DIRTY, readOnlyReason: 'rebase em andamento' }),
    );
    renderOpen();
    expect(await screen.findByText('Este pack abre só para leitura.')).toBeDefined();
    expect(screen.getByText('rebase em andamento')).toBeDefined();
    expect(screen.queryByRole('button', { name: /Limpar/ })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Abrir só para leitura' }));
    await waitFor(() => {
      expect(backend.calls.filter((call) => WRITES.includes(call.command))).toEqual([
        { command: 'pack_import', args: { path: FOLDER, addControls: false } },
      ]);
    });
  });

  it('pack já registrado: explica e oferece outra pasta', async () => {
    const user = userEvent.setup();
    openBackend(makeImportPreview(), {
      pack_import: () => ipcError(makeAppError({ domain: 'project', code: 'ALREADY_REGISTERED' })),
    });
    renderOpen();
    await user.click(await screen.findByRole('button', { name: 'Abrir pack' }));
    expect(await screen.findByText('Este pack já está em Meus packs.')).toBeDefined();
  });

  it('limpeza que falha depois de registrar oferece abrir o pack mesmo assim', async () => {
    const user = userEvent.setup();
    openBackend(makeImportPreview({ hygiene: DIRTY }), {
      pack_hygiene_fix: () =>
        ipcError(makeAppError({ domain: 'project', code: 'PACK_CHANGED_EXTERNALLY' })),
    });
    renderOpen();
    await user.click(await screen.findByRole('button', { name: 'Limpar 3 arquivos e abrir' }));
    expect(await screen.findByText('O pack foi aberto, mas a limpeza não terminou.')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Abrir o pack' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Meu pack antigo' })).toBeDefined();
  });

  it('sem pasta escolhida, oferece escolher', async () => {
    mockBackend();
    renderApp('/packs/abrir');
    expect(
      await screen.findByText('Escolha a pasta de um pack packwiz (a pasta que tem o pack.toml).'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Escolher pasta…' })).toBeDefined();
  });
});
