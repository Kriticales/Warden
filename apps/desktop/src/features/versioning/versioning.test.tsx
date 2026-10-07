/**
 * Salvar versão (SPEC T16) e Histórico (SPEC T17) no app inteiro, com o backend simulado. As
 * regras do histórico (CA-T16-01 a CA-T16-03, CA-T17-01 a CA-T17-03) são provadas no Rust
 * (`warden-versioning`, `commands/versioning.rs`) e no E2E com o git de verdade; aqui ficam as
 * telas: a sugestão e o changelog do diálogo, a recusa do número menor (CA-T16-04), o estado de
 * cada versão (CA-T16-05), voltar, pontos de segurança e descartar.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { HistoryView } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';
import {
  makeChangeSet,
  makeHistory,
  makeItemChange,
  makeSafetyPoint,
  makeSavePreview,
  makeSavedVersion,
  makeUpdated,
} from './versioning.fixtures';

type Options = NonNullable<Parameters<typeof editorBackend>[0]>;

/** Um histórico com três versões (a 1.4.2 final, a 1.4.0 publicada, a 1.3.0 só salva). */
function threeVersions(overrides: Partial<HistoryView> = {}): HistoryView {
  return makeHistory({
    lastVersion: '1.4.2',
    versions: [
      makeSavedVersion('1.4.2', {
        isFinal: true,
        message: '### Mods atualizados\n- Create 0.5.1.i → 0.5.1.j\n',
      }),
      makeSavedVersion('1.4.0', {
        isFinal: true,
        isPublished: true,
        date: '2026-09-20T10:00:00-03:00',
        message: 'Mochilas novas\n\n### Mods adicionados\n- Sophisticated Backpacks 3.20.17\n',
      }),
      makeSavedVersion('1.3.0', { date: '2026-09-10T10:00:00-03:00', message: '' }),
    ],
    ...overrides,
  });
}

async function openHistory(
  history: HistoryView,
  handlers: Options['handlers'] = {},
  pack: Options['pack'] = {},
) {
  const { backend, row } = editorBackend({
    pack: { unsavedFiles: history.unsaved.files.length, version: '1.4.2', ...pack },
    handlers: { history_get: () => history, ...handlers },
  });
  renderApp(`/packs/${row.id}/historico`);
  await screen.findByRole('heading', { level: 1, name: 'Histórico' }, { timeout: 10_000 });
  await screen.findByRole('heading', { level: 2, name: 'Versões' });
  return { backend, row };
}

/** O item da linha do tempo de uma versão. */
function item(version: string): HTMLElement {
  const element = document.getElementById(`versao-${version.replaceAll('.', '-')}`);
  if (element === null) {
    throw new Error(`versão ${version} fora da linha do tempo`);
  }
  return element;
}

/** O item "Agora" (alterações não salvas). */
function nowItem(): HTMLElement {
  const element = screen.getByText('Agora').closest('li');
  if (element === null) {
    throw new Error('item Agora fora da linha do tempo');
  }
  return element;
}

describe('Histórico (T17)', () => {
  it('mostra o estado de cada versão e as ações certas para cada estado; axe', async () => {
    await openHistory(threeVersions());

    const final = within(item('1.4.2'));
    expect(final.getByText('Versão final · não publicada')).toBeDefined();
    expect(final.getByText('1 atualizado')).toBeDefined();
    // Publicar é da V-03: o botão existe, indisponível, com o motivo na dica.
    const publish = final.getByRole('button', { name: 'Publicar versão 1.4.2' });
    expect(publish.getAttribute('aria-disabled')).toBe('true');
    expect(final.getByRole('button', { name: 'Desmarcar versão final' })).toBeDefined();
    // A versão mais nova, com o pack igual a ela, não oferece voltar para si mesma.
    expect(final.queryByRole('button', { name: 'Voltar para a versão 1.4.2' })).toBeNull();

    const published = within(item('1.4.0'));
    expect(published.getByText('Publicada')).toBeDefined();
    expect(published.getByText('Mochilas novas')).toBeDefined();
    expect(published.queryByRole('button', { name: 'Desmarcar versão final' })).toBeNull();
    expect(published.queryByRole('button', { name: /Publicar versão/ })).toBeNull();
    expect(published.getByRole('button', { name: 'Voltar para a versão 1.4.0' })).toBeDefined();

    const saved = within(item('1.3.0'));
    expect(saved.getByText('Só salva')).toBeDefined();
    expect(saved.getByText('Sem mudanças registradas')).toBeDefined();
    expect(saved.getByRole('button', { name: 'Marcar como versão final' })).toBeDefined();

    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o espaço da publicação explica como os jogadores recebem, sem publicar nada', async () => {
    await openHistory(threeVersions());
    const panel = screen.getByRole('region', { name: 'Publicação para os jogadores' });
    expect(panel.textContent).toContain('Este pack ainda não foi publicado.');
  });

  it('alterações não salvas: lista o que mudou e abre Salvar versão', async () => {
    const user = userEvent.setup();
    const unsaved = makeChangeSet({
      items: [
        makeItemChange(),
        makeUpdated('Just Enough Items', '15.20.0.106', '15.20.0.112'),
        makeItemChange({ kind: 'removed', name: 'Old Mod', old: makeItemChange().new, new: null }),
      ],
      configs: ['config/create-common.toml', 'options.txt'],
      minecraft: { old: '1.20.1', new: '1.20.4' },
    });
    const { backend } = await openHistory(threeVersions({ unsaved }), {
      version_save_preview: () => makeSavePreview(),
    });

    const now = within(nowItem());
    expect(now.getByText('Não salvas')).toBeDefined();
    expect(now.getByText('5 alterações desde a 1.4.2')).toBeDefined();
    expect(now.getByText('Minecraft 1.20.1 → 1.20.4')).toBeDefined();
    expect(now.getByText('Sophisticated Backpacks 3.20.17')).toBeDefined();
    expect(now.getByText('Just Enough Items 15.20.0.106 → 15.20.0.112')).toBeDefined();
    expect(now.getByText('config/create-common.toml')).toBeDefined();

    await user.click(now.getByRole('button', { name: 'Salvar versão' }));
    expect(await screen.findByRole('dialog', { name: 'Salvar versão' })).toBeDefined();
    expect(backend.callsOf('version_save_preview')).toHaveLength(1);
  });

  it('sem alterações, diz que nada mudou desde a última versão', async () => {
    await openHistory(threeVersions());
    expect(screen.getByText('Nada mudou desde a versão 1.4.2.')).toBeDefined();
  });

  it('antes da primeira versão, mostra o convite a salvar', async () => {
    await openHistory(makeHistory());
    expect(screen.getByText('Nenhuma versão salva ainda')).toBeDefined();
    expect(screen.getByText('O pack está como foi criado.')).toBeDefined();
  });

  it('CA-T16-05: marcar como versão final chama version_set_final e desmarcar também', async () => {
    const user = userEvent.setup();
    const { backend } = await openHistory(threeVersions(), { version_set_final: () => null });

    await user.click(
      within(item('1.3.0')).getByRole('button', { name: 'Marcar como versão final' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('version_set_final')[0]?.args).toMatchObject({
        version: '1.3.0',
        isFinal: true,
      });
    });
    expect(await screen.findByText('A versão 1.3.0 agora é uma versão final.')).toBeDefined();

    await user.click(within(item('1.4.2')).getByRole('button', { name: 'Desmarcar versão final' }));
    await waitFor(() => {
      expect(backend.callsOf('version_set_final')[1]?.args).toMatchObject({
        version: '1.4.2',
        isFinal: false,
      });
    });
    expect(
      await screen.findByText(
        'A versão 1.4.2 deixou de ser versão final. Nenhum arquivo do pack mudou.',
      ),
    ).toBeDefined();
  });

  it('voltar para esta versão: explica o ponto de segurança e chama version_restore', async () => {
    const user = userEvent.setup();
    const unsaved = makeChangeSet({ items: [makeItemChange()], configs: ['a.toml', 'b.toml'] });
    const { backend } = await openHistory(threeVersions({ unsaved }), {
      version_restore: () => ({
        safetyPoint: makeSafetyPoint(),
        written: [],
        deleted: [],
        leftoverTempFiles: [],
      }),
    });

    await user.click(
      within(item('1.4.0')).getByRole('button', { name: 'Voltar para a versão 1.4.0' }),
    );
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Voltar o pack para a versão 1.4.0?',
    });
    expect(dialog.textContent).toContain('O pack fica exatamente como estava na 1.4.0.');
    expect(dialog.textContent).toContain('Arquivos que não existiam nela são removidos.');
    expect(dialog.textContent).toContain(
      'com 3 alterações não salvas, fica guardado num ponto de segurança',
    );
    expect(dialog.textContent).toContain('O histórico não é apagado.');
    await user.click(within(dialog).getByRole('button', { name: 'Voltar para 1.4.0' }));

    await waitFor(() => {
      const call = backend.callsOf('version_restore')[0];
      expect(call?.args).toMatchObject({ version: '1.4.0' });
      expect(typeof call?.args.utcOffsetMinutes).toBe('number');
    });
    expect(await screen.findByText('O pack voltou para a versão 1.4.0')).toBeDefined();
  });

  it('um erro ao voltar (arquivo em uso) aparece no diálogo e o pack não é dado como restaurado', async () => {
    const user = userEvent.setup();
    await openHistory(threeVersions(), {
      version_restore: () =>
        ipcError(
          makeAppError(
            { domain: 'versioning', code: 'FILE_IN_USE' },
            { params: { path: 'mods/jei.jar' } },
          ),
        ),
    });
    await user.click(
      within(item('1.3.0')).getByRole('button', { name: 'Voltar para a versão 1.3.0' }),
    );
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Voltar para 1.3.0' }));
    expect(await within(dialog).findByText(/mods\/jei\.jar/)).toBeDefined();
    expect(screen.queryByText('O pack voltou para a versão 1.3.0')).toBeNull();
  });

  it('Ver diferenças para o estado atual compara a versão com a pasta de agora', async () => {
    const user = userEvent.setup();
    const { backend } = await openHistory(threeVersions(), {
      version_changes: () =>
        makeChangeSet({
          items: [makeUpdated('Create', '0.5.1.i', '0.5.1.j')],
          configs: ['config/create-common.toml'],
        }),
    });
    // A versão mais nova, sem alterações não salvas, já abre com o changelog à mostra.
    expect(within(item('1.4.2')).getByRole('button', { name: 'Fechar detalhes' })).toBeDefined();
    expect(within(item('1.4.2')).getByLabelText('Changelog da versão 1.4.2').textContent).toContain(
      'Create 0.5.1.i → 0.5.1.j',
    );
    // As outras abrem pelo botão.
    await user.click(within(item('1.4.0')).getByRole('button', { name: 'Ver mudanças' }));
    expect(within(item('1.4.0')).getByLabelText('Changelog da versão 1.4.0').textContent).toContain(
      'Sophisticated Backpacks 3.20.17',
    );

    await user.click(
      within(item('1.4.2')).getByRole('button', { name: 'Ver diferenças para o estado atual' }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Da versão 1.4.2 até agora' });
    expect(await within(dialog).findByText('Create 0.5.1.i → 0.5.1.j')).toBeDefined();
    expect(within(dialog).getByText('config/create-common.toml')).toBeDefined();
    expect(backend.callsOf('version_changes')[0]?.args).toMatchObject({ version: '1.4.2' });
  });

  it('Ver diferenças de um pack igual à versão diz isso', async () => {
    const user = userEvent.setup();
    await openHistory(threeVersions(), { version_changes: () => makeChangeSet() });
    await user.click(
      within(item('1.4.2')).getByRole('button', { name: 'Ver diferenças para o estado atual' }),
    );
    expect(await screen.findByText('O pack está igual à versão 1.4.2.')).toBeDefined();
  });

  it('pontos de segurança: lista com motivo e Recuperar chama safety_point_recover', async () => {
    const user = userEvent.setup();
    const point = makeSafetyPoint();
    const { backend } = await openHistory(threeVersions(), {
      safety_points_list: () => [point],
      safety_point_recover: () => ({
        safetyPoint: makeSafetyPoint({ name: 'outro', reason: 'antes de recuperar' }),
        written: [],
        deleted: [],
        leftoverTempFiles: [],
      }),
    });
    await user.click(screen.getByRole('button', { name: 'Pontos de segurança…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Pontos de segurança' });
    expect(await within(dialog).findByText('antes de voltar para 1.2.0')).toBeDefined();
    await user.click(
      within(dialog).getByRole('button', { name: /^Recuperar o ponto de segurança/ }),
    );
    const confirm = await screen.findByRole('alertdialog');
    expect(confirm.textContent).toContain('antes de voltar para 1.2.0');
    await user.click(within(confirm).getByRole('button', { name: 'Recuperar ponto' }));
    await waitFor(() => {
      expect(backend.callsOf('safety_point_recover')[0]?.args).toMatchObject({ name: point.name });
    });
    expect(await screen.findByText('Ponto de segurança recuperado')).toBeDefined();
  });

  it('sem pontos de segurança, diz que ainda não há', async () => {
    const user = userEvent.setup();
    await openHistory(threeVersions(), { safety_points_list: () => [] });
    await user.click(screen.getByRole('button', { name: 'Pontos de segurança…' }));
    expect(await screen.findByText('Ainda não há pontos de segurança neste pack.')).toBeDefined();
  });

  it('descartar a alteração de uma config pede confirmação e chama unsaved_discard', async () => {
    const user = userEvent.setup();
    const unsaved = makeChangeSet({ configs: ['config/create-common.toml'] });
    const { backend } = await openHistory(threeVersions({ unsaved }), {
      unsaved_discard: () => null,
    });
    await user.click(
      screen.getByRole('button', { name: 'Descartar a alteração de config/create-common.toml' }),
    );
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Descartar a alteração de config/create-common.toml?',
    });
    expect(dialog.textContent).toContain('O que você mudou nele desde então se perde.');
    await user.click(within(dialog).getByRole('button', { name: 'Descartar alteração' }));
    await waitFor(() => {
      expect(backend.callsOf('unsaved_discard')[0]?.args).toMatchObject({
        path: 'config/create-common.toml',
      });
    });
    expect(await screen.findByText('Alteração descartada')).toBeDefined();
  });

  it('antes da primeira versão não há para onde descartar', async () => {
    await openHistory(makeHistory({ unsaved: makeChangeSet({ configs: ['config/a.toml'] }) }));
    expect(screen.queryByRole('button', { name: /^Descartar a alteração/ })).toBeNull();
  });

  it('pack somente leitura: avisa e desliga as escritas', async () => {
    await openHistory(
      threeVersions({ unsaved: makeChangeSet({ configs: ['config/a.toml'] }) }),
      {},
      { readOnlyReason: 'o histórico está no meio de um merge' },
    );
    expect(
      screen.getByText(
        'Este pack não pode ser alterado agora: o histórico está no meio de um merge',
      ),
    ).toBeDefined();
    expect(within(nowItem()).getByRole('button', { name: 'Salvar versão' })).toHaveProperty(
      'disabled',
      true,
    );
    expect(
      within(item('1.3.0')).getByRole('button', { name: 'Marcar como versão final' }),
    ).toHaveProperty('disabled', true);
  });

  it('mostra o erro ao ler o histórico, com Tentar de novo', async () => {
    const user = userEvent.setup();
    let fail = true;
    const { row } = editorBackend({
      pack: { unsavedFiles: 0 },
      handlers: {
        history_get: () => {
          if (fail) {
            return ipcError(makeAppError({ domain: 'versioning', code: 'GIT_FAILED' }));
          }
          return threeVersions();
        },
      },
    });
    renderApp(`/packs/${row.id}/historico`);
    expect(
      await screen.findByText('Não foi possível ler o histórico', {}, { timeout: 10_000 }),
    ).toBeDefined();
    fail = false;
    await user.click(screen.getByRole('button', { name: /Tentar de novo/ }));
    expect(await screen.findByRole('heading', { level: 2, name: 'Versões' })).toBeDefined();
  });

  it('o menu do pack mostra o contador de alterações não salvas no Histórico', async () => {
    await openHistory(threeVersions({ unsaved: makeChangeSet({ configs: ['a', 'b', 'c'] }) }));
    const menu = screen.getByRole('navigation', { name: 'Seções do pack' });
    expect(within(menu).getByLabelText('3 alterações não salvas')).toBeDefined();
    expect(
      within(menu)
        .getByRole('link', { name: /Histórico/ })
        .getAttribute('aria-current'),
    ).toBe('page');
  });
});

describe('Salvar versão (T16)', () => {
  async function openSave(
    preview = makeSavePreview(),
    handlers: Options['handlers'] = {},
    pack: Options['pack'] = {},
  ) {
    const user = userEvent.setup();
    const { backend, row } = editorBackend({
      pack: { unsavedFiles: 5, version: '1.4.2', ...pack },
      handlers: {
        version_save_preview: () => preview,
        version_validate: () => null,
        ...handlers,
      },
    });
    renderApp(`/packs/${row.id}/mods`);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const header = screen.getByRole('banner', { name: 'Pack aberto' });
    await user.click(
      within(header).getByRole('button', { name: 'Salvar versão, 5 alterações não salvas' }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Salvar versão' });
    return { user, backend, dialog, row };
  }

  async function waitSavable(dialog: HTMLElement, version: string) {
    await waitFor(() => {
      expect(
        within(dialog).getByRole('button', { name: `Salvar versão ${version}` }),
      ).toHaveProperty('disabled', false);
    });
  }

  it('CA-T16-01: sugere a versão com o motivo e mostra o changelog automático; axe', async () => {
    const { dialog } = await openSave();
    const field = await within(dialog).findByLabelText('Número da versão');
    expect(field).toHaveProperty('value', '1.5.0');
    expect(
      within(dialog).getByText(
        'Sugerido: 1.5.0, porque você adicionou 2 itens (mods, resource packs ou shaders).',
      ),
    ).toBeDefined();
    const summary = within(dialog).getByLabelText('Resumo automático');
    expect(summary.textContent).toContain('### Mods adicionados');
    expect(summary.textContent).toContain('Just Enough Items 15.20.0.106 → 15.20.0.112');
    expect(summary.textContent).toContain('config/create-common.toml');
    expect(within(dialog).queryByText(/Você removeu mods que podem ter conteúdo/)).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T16-02: remover mod de mundo avisa em Atenção', async () => {
    const { dialog } = await openSave(
      makeSavePreview({
        worldWarning: true,
        suggestion: {
          version: '2.0.0',
          bump: 'major',
          reasons: [{ kind: 'removedWorldMods', count: 1 }],
        },
      }),
    );
    expect(
      await within(dialog).findByText(/Você removeu mods que podem ter conteúdo nos mundos/),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'Sugerido: 2.0.0, porque você removeu 1 mod com blocos ou itens que podem estar nos mundos.',
      ),
    ).toBeDefined();
  });

  it('CA-T16-04: 1.0.0 com a última em 1.2.0 é recusado com explicação e o botão não salva', async () => {
    const { user, dialog, backend } = await openSave(
      makeSavePreview({ lastVersion: '1.2.0', highestVersion: '1.2.0' }),
      {
        version_validate: (args) =>
          args.version === '1.0.0'
            ? ipcError(
                makeAppError(
                  { domain: 'versioning', code: 'VERSION_NOT_GREATER' },
                  { params: { version: '1.0.0', last: '1.2.0' } },
                ),
              )
            : null,
      },
    );
    const field = await within(dialog).findByLabelText('Número da versão');
    await user.clear(field);
    await user.type(field, '1.0.0');
    expect(
      await within(dialog).findByText(
        'A versão 1.0.0 não é maior que a última versão salva, 1.2.0. Escolha um número maior que 1.2.0.',
      ),
    ).toBeDefined();
    const save = within(dialog).getByRole('button', { name: 'Salvar versão 1.0.0' });
    expect(save).toHaveProperty('disabled', true);
    expect(backend.callsOf('version_save')).toHaveLength(0);

    // Voltar para a sugerida libera.
    await user.click(within(dialog).getByRole('button', { name: 'Usar 1.5.0' }));
    await waitSavable(dialog, '1.5.0');
  });

  it('salvar como versão final envia o pedido, avisa e oferece Publicar versão (indisponível)', async () => {
    // Como no backend de verdade: depois de salvar, a prévia passa a dizer que nada mudou, e o
    // aviso de próximos passos aparece mesmo assim.
    let saved = false;
    const { user, dialog, backend } = await openSave(makeSavePreview(), {
      version_save_preview: () =>
        makeSavePreview(saved ? { suggestion: null, changes: makeChangeSet() } : {}),
      version_save: () => {
        saved = true;
        return makeSavedVersion('1.5.0', { isFinal: true });
      },
    });
    await within(dialog).findByLabelText('Número da versão');
    await user.type(within(dialog).getByLabelText(/^Notas/), 'Mochilas novas');
    await user.click(within(dialog).getByRole('checkbox', { name: /Marcar como versão final/ }));
    await waitSavable(dialog, '1.5.0');
    await user.click(within(dialog).getByRole('button', { name: 'Salvar versão 1.5.0' }));

    await waitFor(() => {
      expect(backend.callsOf('version_save')[0]?.args).toMatchObject({
        request: { version: '1.5.0', notes: 'Mochilas novas', markFinal: true },
      });
    });
    const request = backend.callsOf('version_save')[0]?.args.request as {
      utcOffsetMinutes: number;
    };
    expect(typeof request.utcOffsetMinutes).toBe('number');

    const notice = await screen.findByRole('dialog', {
      name: 'Versão 1.5.0 salva como versão final',
    });
    expect(notice.textContent).toContain('Os jogadores só recebem quando você publicar.');
    expect(
      within(notice)
        .getByRole('button', { name: 'Publicar versão 1.5.0' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    expect(
      within(notice).getByRole('link', { name: 'Exportar arquivo' }).getAttribute('href'),
    ).toContain('/exportar');
    // O "Fechar" do rodapé e o ✕ do canto.
    expect(within(notice).getAllByRole('button', { name: 'Fechar' })).toHaveLength(2);
  });

  it('salvar sem marcar como final não oferece Publicar versão', async () => {
    const { user, dialog, backend } = await openSave(makeSavePreview(), {
      version_save: () => makeSavedVersion('1.5.0'),
    });
    await within(dialog).findByLabelText('Número da versão');
    await waitSavable(dialog, '1.5.0');
    await user.click(within(dialog).getByRole('button', { name: 'Salvar versão 1.5.0' }));
    const notice = await screen.findByRole('dialog', { name: 'Versão 1.5.0 salva' });
    expect(notice.textContent).toContain('Nada foi enviado ao GitHub.');
    expect(within(notice).queryByRole('button', { name: /Publicar versão/ })).toBeNull();
    expect(backend.callsOf('version_save')[0]?.args).toMatchObject({
      request: { markFinal: false },
    });
  });

  it('um erro ao salvar fica no diálogo e nada é dado como salvo', async () => {
    const { user, dialog } = await openSave(makeSavePreview(), {
      version_save: () => ipcError(makeAppError({ domain: 'versioning', code: 'NOTHING_CHANGED' })),
    });
    await within(dialog).findByLabelText('Número da versão');
    await waitSavable(dialog, '1.5.0');
    await user.click(within(dialog).getByRole('button', { name: 'Salvar versão 1.5.0' }));
    expect(await within(dialog).findByText(/Nada mudou desde a última versão salva/)).toBeDefined();
    expect(screen.queryByRole('dialog', { name: /^Versão 1\.5\.0 salva/ })).toBeNull();
  });

  it('notas longas demais dizem o limite e impedem salvar', async () => {
    const { dialog } = await openSave();
    await within(dialog).findByLabelText('Número da versão');
    await waitSavable(dialog, '1.5.0');
    fireEvent.change(within(dialog).getByLabelText(/^Notas/), {
      target: { value: 'a'.repeat(20_001) },
    });
    expect(
      await within(dialog).findByText(
        'As notas passam de 20000 caracteres. Encurte o texto para salvar.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Salvar versão 1.5.0' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('sem mudanças, diz que nada mudou desde a última versão e não deixa salvar', async () => {
    const { dialog } = await openSave(makeSavePreview({ suggestion: null }));
    expect(await within(dialog).findByText('Nada mudou desde a versão 1.4.2.')).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: /^Salvar versão \d/ })).toBeNull();
  });

  it('a primeira versão explica que usa a versão do pack', async () => {
    const { dialog } = await openSave(
      makeSavePreview({
        lastVersion: null,
        highestVersion: null,
        suggestion: {
          version: '0.1.0',
          bump: null,
          reasons: [{ kind: 'firstVersion', packVersion: '0.1.0', valid: true }],
        },
      }),
    );
    expect(
      await within(dialog).findByText(
        'Sugerido: 0.1.0, porque é a primeira versão salva e usa a versão que o pack já tem (0.1.0).',
      ),
    ).toBeDefined();
  });

  it('erro ao calcular o que mudou, com Tentar de novo', async () => {
    const user = userEvent.setup();
    let fail = true;
    const { row } = editorBackend({
      pack: { unsavedFiles: 2 },
      handlers: {
        version_save_preview: () => {
          if (fail) {
            return ipcError(makeAppError({ domain: 'versioning', code: 'GIT_FAILED' }));
          }
          return makeSavePreview();
        },
        version_validate: () => null,
      },
    });
    renderApp(`/packs/${row.id}/mods`);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    await user.click(
      screen.getByRole('button', { name: 'Salvar versão, 2 alterações não salvas' }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Salvar versão' });
    expect(await within(dialog).findByText('Não foi possível calcular o que mudou')).toBeDefined();
    fail = false;
    await user.click(within(dialog).getByRole('button', { name: /Tentar de novo/ }));
    // Ao trocar o erro pelo formulário, o diálogo é montado de novo: busca-se outra vez.
    const retried = await screen.findByRole('dialog', { name: 'Salvar versão' });
    expect(await within(retried).findByLabelText('Número da versão')).toBeDefined();
  });

  it('o botão do cabeçalho sem alterações fica indisponível, com o motivo', async () => {
    const { row } = editorBackend({ pack: { unsavedFiles: 0 } });
    renderApp(`/packs/${row.id}/mods`);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const header = screen.getByRole('banner', { name: 'Pack aberto' });
    const button = within(header).getByRole('button', { name: 'Salvar versão' });
    expect(button.getAttribute('aria-disabled')).toBe('true');
    const user = userEvent.setup();
    await user.click(button);
    expect(screen.queryByRole('dialog', { name: 'Salvar versão' })).toBeNull();
  });

  it('o botão do cabeçalho com o pack somente leitura não abre o diálogo', async () => {
    const { row } = editorBackend({
      pack: { unsavedFiles: 3, readOnlyReason: 'rebase em andamento' },
    });
    renderApp(`/packs/${row.id}/mods`);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const user = userEvent.setup();
    const button = screen.getByRole('button', { name: 'Salvar versão, 3 alterações não salvas' });
    expect(button.getAttribute('aria-disabled')).toBe('true');
    await user.click(button);
    expect(screen.queryByRole('dialog', { name: 'Salvar versão' })).toBeNull();
  });
});
